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
	"animedb/internal/postgres"
	"animedb/internal/repository"

	"github.com/go-chi/chi/v5"
	"github.com/go-chi/chi/v5/middleware"
)

const (
	defaultListenAddr         = ":8081"
	defaultAniListDSN         = "postgres://root:root@localhost:5432/anilist?sslmode=disable"
	defaultMyAnimeListDSN     = "postgres://root:root@localhost:5432/myanimelist?sslmode=disable"
	queryTimeout              = 15 * time.Second
	normalizeTitleFunctionSQL = `
CREATE OR REPLACE FUNCTION normalize_title(input TEXT)
RETURNS TEXT
LANGUAGE sql
IMMUTABLE
STRICT
AS $$
	SELECT COALESCE(
		trim(BOTH ' ' FROM regexp_replace(lower(unaccent(input)), '[^a-z0-9]+', ' ', 'g')),
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
					lower(unaccent(input)),
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
	)

	flag.StringVar(&listenAddr, "listen", defaultListenAddr, "HTTP listen address")
	flag.StringVar(&adminDSN, "admin-dsn", "postgres://root:root@localhost:5432/root?sslmode=disable", "Admin Postgres DSN used to ensure target databases exist")
	flag.StringVar(&anilistDSN, "anilist-dsn", defaultAniListDSN, "Postgres DSN for the AniList database")
	flag.StringVar(&myAnimeListDSN, "myanimelist-dsn", defaultMyAnimeListDSN, "Postgres DSN for the MyAnimeList database")
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

	aniHandlers := handlers.NewAniListHandlers(aniDB)
	// TODO: myAnimeListHandlers := handlers.NewMyAnimeListHandlers(malDB)

	router := chi.NewRouter()
	router.Use(middleware.RequestID)
	router.Use(middleware.RealIP)
	router.Use(middleware.Recoverer)

	router.Get("/healthz", func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusOK)
		fmt.Fprint(w, `{"status": "ok"}`)
	})

	router.Route("/anilist", func(r chi.Router) {
		r.Get("/media/search", aniHandlers.MediaSearch)
		r.Get("/media", aniHandlers.MediaList)
		r.Get("/media/{id}", aniHandlers.MediaGet)
	})

	// TODO: Add MyAnimeList routes

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
