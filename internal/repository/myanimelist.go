package repository

import (
	"context"
	"database/sql"
	"fmt"
	"time"
)

func EnsureMyAnimeListSearchHelpers(ctx context.Context, db *sql.DB, normalizeTitleFunctionSQL string) error {
	schemaCtx, cancel := context.WithTimeout(ctx, 5*time.Minute)
	defer cancel()
	
	extensionStatements := []string{
		`CREATE EXTENSION IF NOT EXISTS unaccent;`,
		`CREATE EXTENSION IF NOT EXISTS pg_trgm;`,
	}
	
	if err := EnsureSchemas(schemaCtx, db, extensionStatements); err != nil {
		return err
	}
	
	functionStatements := []string{
		normalizeTitleFunctionSQL,
	}
	
	if err := EnsureSchemas(schemaCtx, db, functionStatements); err != nil {
		return err
	}
	
	exists, err := TableExists(schemaCtx, db, "anime")
	if err != nil {
		return err
	}
	
	if exists {
		indexStatements := []string{
			`CREATE INDEX IF NOT EXISTS anime_title_trgm_idx ON anime USING gin ( normalize_title(COALESCE(title,'')||' '||COALESCE(title_english,'')||' '||COALESCE(title_japanese,'')) gin_trgm_ops );`,
		}
		if err := EnsureSchemas(schemaCtx, db, indexStatements); err != nil {
			return err
		}
	}
	
	return nil
}

func TableExists(ctx context.Context, db *sql.DB, table string) (bool, error) {
	const q = `
SELECT EXISTS (
	SELECT 1
	FROM information_schema.tables
	WHERE table_schema = 'public'
	  AND table_name = $1
);
`
	var exists bool
	if err := db.QueryRowContext(ctx, q, table).Scan(&exists); err != nil {
		return false, fmt.Errorf("check table exists: %w", err)
	}
	return exists, nil
}

func EnsureSchemas(ctx context.Context, db *sql.DB, statements []string) error {
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
