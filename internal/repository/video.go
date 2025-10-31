package repository

import (
	"context"
	"database/sql"
	"time"
)

func EnsureVideosSearchHelpers(ctx context.Context, db *sql.DB, normalizeTitleFunctionSQL string) error {
	schemaCtx, cancel := context.WithTimeout(ctx, 5*time.Minute)
	defer cancel()

	extensionStatements := []string{
		`CREATE EXTENSION IF NOT EXISTS unaccent;`,
		`CREATE EXTENSION IF NOT EXISTS pg_trgm;`,
	}

	if err := ensureSchemas(schemaCtx, db, extensionStatements); err != nil {
		return err
	}

	functionStatements := []string{
		normalizeTitleFunctionSQL,
	}

	if err := ensureSchemas(schemaCtx, db, functionStatements); err != nil {
		return err
	}

	exists, err := TableExists(schemaCtx, db, "anime")
	if err != nil {
		return err
	}

	if exists {
		indexStatements := []string{
			`CREATE INDEX IF NOT EXISTS anime_title_trgm_idx ON anime USING gin ( normalize_title(title) gin_trgm_ops );`,
		}
		if err := ensureSchemas(schemaCtx, db, indexStatements); err != nil {
			return err
		}
	}

	return nil
}

