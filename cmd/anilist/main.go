package main

import (
	"context"
	"database/sql"
	"flag"
	"log"
	"os"
	"os/signal"
	"syscall"

	"animedb/internal/ingest/anilist"
	"animedb/internal/postgres"

	_ "github.com/lib/pq"
)

const (
	defaultAdminDSN = "postgres://root:root@localhost:5432/root?sslmode=disable"
	defaultDatabase = "anilist"
)

const normalizeTitleSQL = `
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
`

var schemaStatements = []string{
	`CREATE EXTENSION IF NOT EXISTS unaccent;`,
	`CREATE EXTENSION IF NOT EXISTS pg_trgm;`,
	normalizeTitleSQL,
	`CREATE TABLE IF NOT EXISTS media (
		id INTEGER PRIMARY KEY,
		type TEXT,
		title_romaji TEXT,
		title_english TEXT,
		title_native TEXT,
		synonyms TEXT[],
		description TEXT,
		format TEXT,
		status TEXT,
		episodes INTEGER,
		duration INTEGER,
		country_of_origin TEXT,
		source TEXT,
		season TEXT,
		season_year INTEGER,
		average_score INTEGER,
		mean_score INTEGER,
		popularity INTEGER,
		favourites INTEGER,
		genres TEXT[],
		tags JSONB,
		studios JSONB,
		start_date_year INTEGER,
		start_date_month INTEGER,
		start_date_day INTEGER,
		end_date_year INTEGER,
		end_date_month INTEGER,
		end_date_day INTEGER,
		cover_image TEXT,
		banner_image TEXT,
		updated_at TIMESTAMPTZ,
		site_url TEXT,
		is_adult BOOLEAN,
		is_licensed BOOLEAN
	);`,
	`ALTER TABLE media
ADD COLUMN IF NOT EXISTS normalized_title TEXT GENERATED ALWAYS AS (
	normalize_title(
		COALESCE(title_romaji, '') || ' ' ||
		COALESCE(title_english, '') || ' ' ||
		COALESCE(title_native, '')
	)
) STORED;`,
	`CREATE INDEX IF NOT EXISTS media_season_year_idx ON media (season_year);`,
	`CREATE INDEX IF NOT EXISTS media_type_idx ON media (type);`,
	`CREATE INDEX IF NOT EXISTS media_normalized_title_trgm_idx ON media USING gin (normalized_title gin_trgm_ops);`,
}

func main() {
	ctx, stop := signal.NotifyContext(context.Background(), os.Interrupt, syscall.SIGTERM)
	defer stop()

	var (
		adminDSN  string
		database  string
		perPage   int
		startPage int
		maxPages  int
		mediaType string
	)

	flag.StringVar(&adminDSN, "dsn", defaultAdminDSN, "Admin Postgres DSN that has privileges to create databases")
	flag.StringVar(&database, "database", defaultDatabase, "Target database name to create/populate")
	flag.IntVar(&perPage, "per-page", 50, "Number of media records to pull per request (max 50)")
	flag.IntVar(&startPage, "start-page", 1, "Starting Page value")
	flag.IntVar(&maxPages, "max-pages", 0, "Maximum number of pages to fetch (0 fetches all)")
	flag.StringVar(&mediaType, "media-type", "ANIME", "AniList MediaType filter (ANIME or MANGA)")
	flag.Parse()

	log.SetFlags(log.LstdFlags | log.Lmicroseconds)

	targetDSN, err := postgres.EnsureDatabase(ctx, adminDSN, database)
	if err != nil {
		log.Fatalf("ensure database: %v", err)
	}

	db, err := sql.Open("postgres", targetDSN)
	if err != nil {
		log.Fatalf("connect to anilist database: %v", err)
	}
	defer db.Close()

	if err := postgres.EnsureSchemas(ctx, db, schemaStatements); err != nil {
		log.Fatalf("ensure schema: %v", err)
	}

	client := anilist.NewClient()
	persister := anilist.NewPersister(db)

	if startPage < 1 {
		startPage = 1
	}

	var totalInserted int
	page := startPage

	for {
		if maxPages > 0 && (page-startPage) >= maxPages {
			break
		}

		select {
		case <-ctx.Done():
			log.Printf("context cancelled, stopping ingestion: %v", ctx.Err())
			return
		default:
		}

		log.Printf("Fetching AniList page %d (perPage=%d)...", page, perPage)
		resp, err := client.FetchPage(ctx, page, perPage, mediaType)
		if err != nil {
			log.Fatalf("fetch page %d: %v", page, err)
		}

		if len(resp.Data.Page.Media) == 0 {
			log.Printf("No more media returned at page %d, stopping.", page)
			break
		}

		inserted, err := persister.PersistMedia(ctx, resp.Data.Page.Media)
		if err != nil {
			log.Fatalf("persist page %d: %v", page, err)
		}
		totalInserted += inserted

		log.Printf("Stored %d media entries (total so far: %d)", inserted, totalInserted)

		if !resp.Data.Page.PageInfo.HasNextPage {
			break
		}
		page++
	}

	log.Printf("AniList ingestion complete. Total rows upserted: %d", totalInserted)
}
