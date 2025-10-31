package main

import (
	"context"
	"database/sql"
	"flag"
	"fmt"
	"log"
	"net/http"
	"strings"
	"time"

	"animedb/internal/http/handlers"
	custommiddleware "animedb/internal/middleware"
	"animedb/internal/postgres"
	"animedb/internal/repository"

	"github.com/go-chi/chi/v5"
	"github.com/go-chi/chi/v5/middleware"
)

const (
	defaultListenAddr         = ":8081"
	defaultAniListDSN         = "postgres://root:root@localhost:5432/anilist?sslmode=disable"
	defaultMyAnimeListDSN     = "postgres://root:root@localhost:5432/myanimelist?sslmode=disable"
	defaultVideosDSN          = "postgres://root:root@localhost:5432/videos?sslmode=disable"
	queryTimeout              = 15 * time.Second
	normalizeTitleFunctionSQL = `
CREATE OR REPLACE FUNCTION normalize_title(input TEXT)
RETURNS TEXT
LANGUAGE sql
IMMUTABLE
STRICT
AS $$
	SELECT COALESCE(
		trim(BOTH ' ' FROM regexp_replace(lower(public.unaccent(input)), '[^a-z0-9]+', ' ', 'g')),
		''
	);
$$;

CREATE OR REPLACE FUNCTION normalize_title_preserve_season(input TEXT)
RETURNS TEXT
LANGUAGE sql
IMMUTABLE
STRICT
AS $$
	SELECT COALESCE(
		trim(BOTH ' ' FROM
			regexp_replace(
				regexp_replace(
					lower(public.unaccent(input)),
					'(\d+)(st|nd|rd|th)\s*season',
					'season \1',
					'gi'
				),
				'[^a-z0-9]+',
				' ',
				'g'
			)
		),
		''
	);
$$;
`
)

func main() {
	var (
		listenAddr     string
		adminDSN       string
		anilistDSN     string
		myAnimeListDSN string
		videosDSN      string
		scanPath       string
	)

	flag.StringVar(&listenAddr, "listen", defaultListenAddr, "HTTP listen address")
	flag.StringVar(&adminDSN, "admin-dsn", "postgres://root:root@localhost:5432/root?sslmode=disable", "Admin Postgres DSN used to ensure target databases exist")
	flag.StringVar(&anilistDSN, "anilist-dsn", defaultAniListDSN, "Postgres DSN for the AniList database")
	flag.StringVar(&myAnimeListDSN, "myanimelist-dsn", defaultMyAnimeListDSN, "Postgres DSN for the MyAnimeList database")
	flag.StringVar(&videosDSN, "videos-dsn", defaultVideosDSN, "Postgres DSN for the videos database")
	flag.StringVar(&scanPath, "scan-path", "", "Root directory to scan for video files (optional)")
	flag.Parse()

	args := flag.Args()
	if len(args) > 0 {
		listenAddr = args[0]
	}
	if len(args) > 1 {
		anilistDSN = args[1]
	}
	if len(args) > 2 {
		myAnimeListDSN = args[2]
	}

	ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
	defer cancel()

	aniTargetDSN, err := ensureDSN(ctx, adminDSN, anilistDSN, "anilist")
	if err != nil {
		log.Fatalf("prepare AniList database: %v", err)
	}

	malTargetDSN, err := ensureDSN(ctx, adminDSN, myAnimeListDSN, "myanimelist")
	if err != nil {
		log.Fatalf("prepare MyAnimeList database: %v", err)
	}

	vidTargetDSN, err := ensureDSN(ctx, adminDSN, videosDSN, "videos")
	if err != nil {
		log.Fatalf("prepare videos database: %v", err)
	}

	aniDB, err := openAndPing(aniTargetDSN)
	if err != nil {
		log.Fatalf("connect AniList database: %v", err)
	}
	defer aniDB.Close()
	if err := repository.EnsureAniListSearchHelpers(context.Background(), aniDB, normalizeTitleFunctionSQL); err != nil {
		log.Fatalf("ensure AniList search helpers: %v", err)
	}

	malDB, err := openAndPing(malTargetDSN)
	if err != nil {
		log.Fatalf("connect MyAnimeList database: %v", err)
	}
	defer malDB.Close()
	if err := repository.EnsureMyAnimeListSearchHelpers(context.Background(), malDB, normalizeTitleFunctionSQL); err != nil {
		log.Fatalf("ensure MyAnimeList search helpers: %v", err)
	}

	vidDB, err := openAndPing(vidTargetDSN)
	if err != nil {
		log.Fatalf("connect videos database: %v", err)
	}
	defer vidDB.Close()
	if err := repository.EnsureVideosSearchHelpers(context.Background(), vidDB, normalizeTitleFunctionSQL); err != nil {
		log.Fatalf("ensure videos search helpers: %v", err)
	}

	aniRepo := repository.NewAniListRepository(aniDB)
	malRepo := repository.NewMyAnimeListRepository(malDB)
	vidRepo := repository.NewVideoRepository(vidDB)

	aniHandlers := handlers.NewAniListHandlers(aniRepo)
	myAnimeListHandlers := handlers.NewMyAnimeListHandlers(malRepo)
	realtimeHandlers := handlers.NewRealtimeSearchHandlers(aniRepo, malRepo)
	videoHandlers := handlers.NewVideoHandlers(vidRepo)

	rateLimiter := custommiddleware.NewRateLimiter(100)

	router := chi.NewRouter()
	router.Use(middleware.RequestID)
	router.Use(middleware.RealIP)
	router.Use(middleware.Recoverer)
	router.Use(custommiddleware.ValidationMiddleware)
	router.Use(rateLimiter.Middleware)

	router.Get("/healthz", func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusOK)
		fmt.Fprint(w, `{"status": "ok"}`)
	})

	router.Get("/search/realtime", realtimeHandlers.Search)

	router.Route("/anilist", func(r chi.Router) {
		r.Get("/media/search", aniHandlers.MediaSearch)
		r.Get("/media", aniHandlers.MediaList)
		r.Get("/media/{id}", aniHandlers.MediaGet)
	})

	router.Route("/myanimelist", func(r chi.Router) {
		r.Get("/anime/search", myAnimeListHandlers.MediaSearch)
		r.Get("/anime", myAnimeListHandlers.MediaList)
		r.Get("/anime/{id}", myAnimeListHandlers.MediaGet)
	})

	router.Route("/videos", func(r chi.Router) {
		r.Get("/anime", videoHandlers.AnimeList)
		r.Get("/anime/{id}", videoHandlers.AnimeGet)
		r.Get("/anime/{id}/episodes", videoHandlers.EpisodesList)
		r.Get("/episodes/{id}", videoHandlers.EpisodeGet)
		r.Get("/episodes/{id}/thumbnails", videoHandlers.ThumbnailsList)
		r.Get("/search", videoHandlers.Search)
		r.Post("/scan", videoHandlers.TriggerScan)
		r.Get("/scan/status", videoHandlers.ScanStatus)
	})

	httpServer := &http.Server{
		Addr:              listenAddr,
		Handler:           router,
		ReadHeaderTimeout: 10 * time.Second,
	}

	log.Printf("REST API listening on %s", listenAddr)
	if err := httpServer.ListenAndServe(); err != nil && err != http.ErrServerClosed {
		log.Fatalf("http server error: %v", err)
	}
}

func ensureDSN(ctx context.Context, adminDSN, providedDSN, database string) (string, error) {
	if strings.Contains(providedDSN, fmt.Sprintf("/%s", database)) || strings.Contains(providedDSN, fmt.Sprintf("dbname=%s", database)) {
		if _, err := postgres.EnsureDatabase(ctx, adminDSN, database); err != nil {
			return "", err
		}
		return providedDSN, nil
	}
	return postgres.EnsureDatabase(ctx, adminDSN, database)
}

func openAndPing(dsn string) (*sql.DB, error) {
	db, err := sql.Open("postgres", dsn)
	if err != nil {
		return nil, err
	}
	ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
	defer cancel()
	if err := db.PingContext(ctx); err != nil {
		db.Close()
		return nil, err
	}
	return db, nil
}
