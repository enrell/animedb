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
	
	baseStatements := []string{
		`CREATE EXTENSION IF NOT EXISTS unaccent;`,
		`CREATE EXTENSION IF NOT EXISTS pg_trgm;`,
		normalizeTitleFunctionSQL,
	}
	
	if err := ensureSchemas(schemaCtx, db, baseStatements); err != nil {
		return err
	}
	
	exists, err := TableExists(schemaCtx, db, "media")
	if err != nil {
		return err
	}
	
	if exists {
		indexStatements := []string{
			`CREATE INDEX IF NOT EXISTS media_title_trgm_idx ON media USING gin ( normalize_title(COALESCE(title_romaji,'')||' '||COALESCE(title_english,'')||' '||COALESCE(title_native,'')) gin_trgm_ops );`,
		}
		if err := ensureSchemas(schemaCtx, db, indexStatements); err != nil {
			return err
		}
	}
	
	return nil
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
