package main

import (
	"context"
	"database/sql"
	"flag"
	"log"
	"os"
	"os/signal"
	"syscall"

	"animedb/internal/ingest/myanimelist"
	"animedb/internal/postgres"

	_ "github.com/lib/pq"
)

const (
	defaultAdminDSN = "postgres://root:root@localhost:5432/root?sslmode=disable"
	defaultDatabase = "myanimelist"
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
	`CREATE TABLE IF NOT EXISTS anime (
		mal_id INTEGER PRIMARY KEY,
		title TEXT,
		title_english TEXT,
		title_japanese TEXT,
		type TEXT,
		source TEXT,
		episodes INTEGER,
		status TEXT,
		airing BOOLEAN,
		aired_from TIMESTAMPTZ,
		aired_to TIMESTAMPTZ,
		duration TEXT,
		rating TEXT,
		score DOUBLE PRECISION,
		scored_by INTEGER,
		rank INTEGER,
		popularity INTEGER,
		members INTEGER,
		favorites INTEGER,
		synopsis TEXT,
		background TEXT,
		season TEXT,
		year INTEGER,
		broadcast JSONB,
		titles JSONB,
		images JSONB,
		trailer JSONB,
		producers JSONB,
		licensors JSONB,
		studios JSONB,
		genres JSONB,
		themes JSONB,
		demographics JSONB,
		raw JSONB
	);`,
	`CREATE INDEX IF NOT EXISTS anime_year_idx ON anime (year);`,
	`CREATE INDEX IF NOT EXISTS anime_season_idx ON anime (season);`,
	`CREATE INDEX IF NOT EXISTS anime_title_trgm_idx ON anime USING gin ( normalize_title(COALESCE(title,'')||' '||COALESCE(title_english,'')||' '||COALESCE(title_japanese,'')) gin_trgm_ops );`,
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
	)

	flag.StringVar(&adminDSN, "dsn", defaultAdminDSN, "Admin Postgres DSN that has privileges to create databases")
	flag.StringVar(&database, "database", defaultDatabase, "Target database name to create/populate")
	flag.IntVar(&perPage, "per-page", 25, "Number of anime entries per request (max 25)")
	flag.IntVar(&startPage, "start-page", 1, "Starting page")
	flag.IntVar(&maxPages, "max-pages", 0, "Maximum pages to fetch (0 fetches all)")
	flag.Parse()

	log.SetFlags(log.LstdFlags | log.Lmicroseconds)

	targetDSN, err := postgres.EnsureDatabase(ctx, adminDSN, database)
	if err != nil {
		log.Fatalf("ensure database: %v", err)
	}

	db, err := sql.Open("postgres", targetDSN)
	if err != nil {
		log.Fatalf("connect to myanimelist database: %v", err)
	}
	defer db.Close()

	if err := postgres.EnsureSchemas(ctx, db, schemaStatements); err != nil {
		log.Fatalf("ensure schema: %v", err)
	}

	client := myanimelist.NewClient()
	persister := myanimelist.NewPersister(db)

	if startPage < 1 {
		startPage = 1
	}

	page := startPage
	total := 0

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

		log.Printf("Fetching Jikan page %d (perPage=%d)...", page, perPage)
		resp, err := client.FetchPage(ctx, page, perPage)
		if err != nil {
			log.Fatalf("fetch page %d: %v", page, err)
		}

		if len(resp.Data) == 0 {
			log.Printf("No more anime returned at page %d, stopping.", page)
			break
		}

		items, err := myanimelist.DecodeAnime(resp.Data)
		if err != nil {
			log.Fatalf("decode response data: %v", err)
		}

		inserted, err := persister.PersistAnime(ctx, items)
		if err != nil {
			log.Fatalf("persist page %d: %v", page, err)
		}
		total += inserted

		log.Printf("Stored %d anime entries (total so far: %d)", inserted, total)

		if !resp.Pagination.HasNextPage {
			break
		}

		page++
	}

	log.Printf("MyAnimeList ingestion complete. Total rows upserted: %d", total)
}
