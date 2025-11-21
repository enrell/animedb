package main

import (
	"context"
	"database/sql"
	"errors"
	"flag"
	"fmt"
	"log"
	"math/rand"
	"net/url"
	"os"
	"os/exec"
	"os/signal"
	"strings"
	"syscall"
	"time"

	_ "github.com/lib/pq"
)

const defaultAdminDSN = "postgres://root:root@localhost:5432/root?sslmode=disable"

type options struct {
	dsn            string
	anilistPages   int
	anilistPerPage int
	malPages       int
	malPerPage     int
}

func main() {
	ctx, stop := signal.NotifyContext(context.Background(), os.Interrupt, syscall.SIGTERM)
	defer stop()

	var opts options
	flag.StringVar(&opts.dsn, "dsn", defaultAdminDSN, "Admin Postgres DSN used for provisioning temporary databases")
	flag.IntVar(&opts.anilistPages, "anilist-pages", 1, "Number of AniList pages to fetch during the test")
	flag.IntVar(&opts.anilistPerPage, "anilist-per-page", 1, "AniList page size for the test run")
	flag.IntVar(&opts.malPages, "mal-pages", 1, "Number of MyAnimeList pages to fetch during the test")
	flag.IntVar(&opts.malPerPage, "mal-per-page", 1, "MyAnimeList page size for the test run")
	flag.Parse()

	log.SetFlags(log.LstdFlags | log.Lmicroseconds)

	if err := run(ctx, opts); err != nil {
		log.Fatalf("ingestion test failed: %v", err)
	}

	log.Println("ingestion test passed: temporary databases populated and removed successfully")
}

func run(ctx context.Context, opts options) (err error) {
	rng := rand.New(rand.NewSource(time.Now().UnixNano()))

	anilistDB := fmt.Sprintf("anilist_test_%d_%d", time.Now().UnixNano(), rng.Intn(1000))
	malDB := fmt.Sprintf("myanimelist_test_%d_%d", time.Now().UnixNano(), rng.Intn(1000))

	defer func() {
		if derr := dropDatabase(ctx, opts.dsn, malDB); derr != nil {
			err = errors.Join(err, fmt.Errorf("drop database %s: %w", malDB, derr))
		}
		if derr := dropDatabase(ctx, opts.dsn, anilistDB); derr != nil {
			err = errors.Join(err, fmt.Errorf("drop database %s: %w", anilistDB, derr))
		}
	}()

	if err := runGoCommand(ctx, "./cmd/anilist",
		fmt.Sprintf("--dsn=%s", opts.dsn),
		fmt.Sprintf("--database=%s", anilistDB),
		fmt.Sprintf("--per-page=%d", clamp(opts.anilistPerPage, 1, 50)),
		fmt.Sprintf("--max-pages=%d", max(opts.anilistPages, 1)),
	); err != nil {
		return fmt.Errorf("run AniList ingestor: %w", err)
	}

	anilistDSN, err := buildDatabaseDSN(opts.dsn, anilistDB)
	if err != nil {
		return fmt.Errorf("build AniList DSN: %w", err)
	}
	if err := assertRowExists(ctx, anilistDSN, "SELECT COUNT(*) FROM media WHERE id IS NOT NULL"); err != nil {
		return fmt.Errorf("validate AniList ingestion: %w", err)
	}

	if err := runGoCommand(ctx, "./cmd/myanimelist",
		fmt.Sprintf("--dsn=%s", opts.dsn),
		fmt.Sprintf("--database=%s", malDB),
		fmt.Sprintf("--per-page=%d", clamp(opts.malPerPage, 1, 25)),
		fmt.Sprintf("--max-pages=%d", max(opts.malPages, 1)),
	); err != nil {
		return fmt.Errorf("run MyAnimeList ingestor: %w", err)
	}

	myAnimeListDSN, err := buildDatabaseDSN(opts.dsn, malDB)
	if err != nil {
		return fmt.Errorf("build MyAnimeList DSN: %w", err)
	}
	if err := assertRowExists(ctx, myAnimeListDSN, "SELECT COUNT(*) FROM anime WHERE mal_id IS NOT NULL"); err != nil {
		return fmt.Errorf("validate MyAnimeList ingestion: %w", err)
	}

	return nil
}

func runGoCommand(ctx context.Context, target string, args ...string) error {
	cmdArgs := append([]string{"run", target}, args...)
	cmd := exec.CommandContext(ctx, "go", cmdArgs...)
	cmd.Stdout = os.Stdout
	cmd.Stderr = os.Stderr
	cmd.Env = os.Environ()
	return cmd.Run()
}

func assertRowExists(ctx context.Context, dsn, query string) error {
	db, err := sql.Open("postgres", dsn)
	if err != nil {
		return fmt.Errorf("open database: %w", err)
	}
	defer db.Close()

	ctx, cancel := context.WithTimeout(ctx, 10*time.Second)
	defer cancel()

	var count int
	if err := db.QueryRowContext(ctx, query).Scan(&count); err != nil {
		return fmt.Errorf("execute validation query: %w", err)
	}
	if count == 0 {
		return fmt.Errorf("validation query returned zero rows")
	}
	return nil
}

func dropDatabase(ctx context.Context, adminDSN, database string) error {
	if strings.TrimSpace(database) == "" {
		return nil
	}
	db, err := sql.Open("postgres", adminDSN)
	if err != nil {
		return fmt.Errorf("open admin connection: %w", err)
	}
	defer db.Close()

	ctx, cancel := context.WithTimeout(ctx, 10*time.Second)
	defer cancel()

	_, _ = db.ExecContext(ctx, `SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE datname = $1`, database)

	stmt := fmt.Sprintf(`DROP DATABASE IF EXISTS %s WITH (FORCE)`, quoteIdentifier(database))
	if _, err := db.ExecContext(ctx, stmt); err != nil {
		// Fallback for PostgreSQL versions prior to 13 that do not support WITH (FORCE).
		if !strings.Contains(strings.ToLower(err.Error()), "syntax error") {
			return fmt.Errorf("drop database: %w", err)
		}
		stmt = fmt.Sprintf(`DROP DATABASE IF EXISTS %s`, quoteIdentifier(database))
		if _, dropErr := db.ExecContext(ctx, stmt); dropErr != nil {
			return fmt.Errorf("drop database (fallback): %w", dropErr)
		}
	}
	return nil
}

func buildDatabaseDSN(adminDSN, database string) (string, error) {
	u, err := url.Parse(adminDSN)
	if err != nil {
		return "", err
	}
	query := u.Query()
	query.Set("dbname", database)
	u.RawQuery = query.Encode()
	u.Path = "/" + database
	return u.String(), nil
}

func quoteIdentifier(identifier string) string {
	return `"` + strings.ReplaceAll(identifier, `"`, `""`) + `"`
}

func clamp(value, minValue, maxValue int) int {
	if value < minValue {
		return minValue
	}
	if value > maxValue {
		return maxValue
	}
	return value
}

func max(a, b int) int {
	if a > b {
		return a
	}
	return b
}
