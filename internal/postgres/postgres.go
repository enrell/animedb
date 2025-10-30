package postgres

import (
	"context"
	"database/sql"
	"fmt"
	"net/url"
	"strings"
	"time"

	_ "github.com/lib/pq"
)

// EnsureDatabase ensures that the provided database name exists, creating it when missing.
// It returns a DSN that points to the requested database.
func EnsureDatabase(ctx context.Context, adminDSN, databaseName string) (string, error) {
	if databaseName == "" {
		return "", fmt.Errorf("database name is required")
	}

	adminDB, err := sql.Open("postgres", adminDSN)
	if err != nil {
		return "", fmt.Errorf("open admin connection: %w", err)
	}
	defer adminDB.Close()

	// Ensure the admin connection is alive before proceeding.
	if err := pingWithRetry(ctx, adminDB); err != nil {
		return "", fmt.Errorf("ping admin connection: %w", err)
	}

	var existing string
	err = adminDB.QueryRowContext(ctx, `SELECT datname FROM pg_database WHERE datname = $1`, databaseName).
		Scan(&existing)
	switch {
	case err == sql.ErrNoRows:
		createStmt := fmt.Sprintf("CREATE DATABASE %s", quoteIdentifier(databaseName))
		if _, err := adminDB.ExecContext(ctx, createStmt); err != nil {
			return "", fmt.Errorf("create database %s: %w", databaseName, err)
		}
	case err != nil:
		return "", fmt.Errorf("check database existence: %w", err)
	}

	targetDSN, err := replaceDatabase(adminDSN, databaseName)
	if err != nil {
		return "", fmt.Errorf("build database DSN: %w", err)
	}

	// Touch the target connection to ensure it is reachable and the migrations can run afterwards.
	targetDB, err := sql.Open("postgres", targetDSN)
	if err != nil {
		return "", fmt.Errorf("open target connection: %w", err)
	}
	defer targetDB.Close()

	if err := pingWithRetry(ctx, targetDB); err != nil {
		return "", fmt.Errorf("ping target connection: %w", err)
	}

	return targetDSN, nil
}

// EnsureSchemas executes each statement in order, typically used to create tables or add indexes.
func EnsureSchemas(ctx context.Context, db *sql.DB, statements []string) error {
	for _, stmt := range statements {
		if strings.TrimSpace(stmt) == "" {
			continue
		}
		if _, err := db.ExecContext(ctx, stmt); err != nil {
			return fmt.Errorf("exec schema statement %q: %w", stmt, err)
		}
	}
	return nil
}

func replaceDatabase(rawDSN, databaseName string) (string, error) {
	u, err := url.Parse(rawDSN)
	if err != nil {
		return "", err
	}

	// Prefer explicit dbname query parameter.
	query := u.Query()
	query.Set("dbname", databaseName)
	u.RawQuery = query.Encode()

	// Update the path component as well for URL based connection strings.
	u.Path = "/" + databaseName

	return u.String(), nil
}

func quoteIdentifier(identifier string) string {
	return `"` + strings.ReplaceAll(identifier, `"`, `""`) + `"`
}

func pingWithRetry(ctx context.Context, db *sql.DB) error {
	const (
		maxAttempts    = 5
		initialBackoff = 250 * time.Millisecond
	)

	backoff := initialBackoff
	for attempt := 0; attempt < maxAttempts; attempt++ {
		if err := db.PingContext(ctx); err != nil {
			select {
			case <-ctx.Done():
				return ctx.Err()
			case <-time.After(backoff):
				backoff *= 2
				continue
			}
		}
		return nil
	}
	return fmt.Errorf("database ping timeout")
}
