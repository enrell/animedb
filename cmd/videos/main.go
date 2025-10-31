package main

import (
	"context"
	"database/sql"
	"flag"
	"log"
	"os"
	"os/signal"
	"syscall"

	"animedb/internal/postgres"

	_ "github.com/lib/pq"
)

const (
	defaultAdminDSN = "postgres://root:root@localhost:5432/root?sslmode=disable"
	defaultDatabase  = "videos"
)

const normalizeTitleSQL = `
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
`

var schemaStatements = []string{
	`CREATE EXTENSION IF NOT EXISTS unaccent;`,
	`CREATE EXTENSION IF NOT EXISTS pg_trgm;`,
	normalizeTitleSQL,
	`CREATE TABLE IF NOT EXISTS anime (
		id SERIAL PRIMARY KEY,
		title TEXT NOT NULL,
		folder_path TEXT NOT NULL UNIQUE,
		created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
		updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
	);`,
	`CREATE TABLE IF NOT EXISTS episodes (
		id SERIAL PRIMARY KEY,
		anime_id INTEGER NOT NULL REFERENCES anime(id) ON DELETE CASCADE,
		file_path TEXT NOT NULL UNIQUE,
		filename TEXT NOT NULL,
		file_size BIGINT NOT NULL,
		duration DOUBLE PRECISION,
		hash TEXT NOT NULL,
		format TEXT,
		resolution TEXT,
		episode_number INTEGER,
		season_number INTEGER,
		is_corrupted BOOLEAN NOT NULL DEFAULT FALSE,
		is_partial BOOLEAN NOT NULL DEFAULT FALSE,
		created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
		updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
		indexed_at TIMESTAMPTZ
	);`,
	`CREATE TABLE IF NOT EXISTS thumbnails (
		id SERIAL PRIMARY KEY,
		episode_id INTEGER NOT NULL REFERENCES episodes(id) ON DELETE CASCADE,
		file_path TEXT NOT NULL UNIQUE,
		timestamp_sec DOUBLE PRECISION NOT NULL,
		created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
	);`,
	`CREATE INDEX IF NOT EXISTS anime_title_trgm_idx ON anime USING gin ( normalize_title(title) gin_trgm_ops );`,
	`CREATE INDEX IF NOT EXISTS episodes_anime_id_idx ON episodes (anime_id);`,
	`CREATE INDEX IF NOT EXISTS episodes_hash_idx ON episodes (hash);`,
	`CREATE INDEX IF NOT EXISTS episodes_file_path_idx ON episodes (file_path);`,
	`CREATE INDEX IF NOT EXISTS thumbnails_episode_id_idx ON thumbnails (episode_id);`,
}

func main() {
	ctx, stop := signal.NotifyContext(context.Background(), os.Interrupt, syscall.SIGTERM)
	defer stop()

	var (
		adminDSN string
		database string
	)

	flag.StringVar(&adminDSN, "dsn", defaultAdminDSN, "Admin Postgres DSN that has privileges to create databases")
	flag.StringVar(&database, "database", defaultDatabase, "Target database name to create/populate")
	flag.Parse()

	log.SetFlags(log.LstdFlags | log.Lmicroseconds)

	targetDSN, err := postgres.EnsureDatabase(ctx, adminDSN, database)
	if err != nil {
		log.Fatalf("ensure database: %v", err)
	}

	db, err := sql.Open("postgres", targetDSN)
	if err != nil {
		log.Fatalf("connect to videos database: %v", err)
	}
	defer db.Close()

	if err := postgres.EnsureSchemas(ctx, db, schemaStatements); err != nil {
		log.Fatalf("ensure schema: %v", err)
	}

	log.Printf("Video database schema setup complete for database: %s", database)
}

