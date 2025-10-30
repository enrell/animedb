package repository

import (
	"context"
	"database/sql"
	"fmt"
	"time"
)

func EnsureAniListSearchHelpers(ctx context.Context, db *sql.DB, normalizeTitleFunctionSQL string) error {
	schemaCtx, cancel := context.WithTimeout(ctx, 5*time.Minute)
	defer cancel()
	statements := []string{
		`CREATE EXTENSION IF NOT EXISTS unaccent;`,
		`CREATE EXTENSION IF NOT EXISTS pg_trgm;`,
		normalizeTitleFunctionSQL,
	}
	exists, err := TableExists(schemaCtx, db, "media")
	if err != nil {
		return err
	}
	if exists {
		statements = append(statements,
			`ALTER TABLE IF EXISTS media
	ADD COLUMN IF NOT EXISTS normalized_title TEXT GENERATED ALWAYS AS (
		normalize_title(
			COALESCE(title_romaji, '') || ' ' ||
			COALESCE(title_english, '') || ' ' ||
			COALESCE(title_native, '')
		)
	) STORED;`,
			`CREATE INDEX IF NOT EXISTS media_normalized_title_trgm_idx ON media USING gin (normalized_title gin_trgm_ops);`,
		)
	}
	return ensureSchemas(schemaCtx, db, statements)
}

func ensureSchemas(ctx context.Context, db *sql.DB, statements []string) error {
	for _, stmt := range statements {
		if len(stmt) == 0 {
			continue
		}
		if _, err := db.ExecContext(ctx, stmt); err != nil {
			return fmt.Errorf("exec schema statement %q: %w", stmt, err)
		}
	}
	return nil
}
