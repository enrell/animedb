package main

import (
	"context"
	"database/sql"
	"encoding/json"
	"errors"
	"flag"
	"fmt"
	"io"
	"log"
	"net/http"
	"os"
	"os/signal"
	"strconv"
	"strings"
	"syscall"
	"time"

	"animedb/internal/postgres"
	"animedb/internal/ratelimit"

	_ "github.com/lib/pq"
)

const (
	defaultMyAnimeListAdminDSN = "postgres://root:root@localhost:5432/root?sslmode=disable"
	defaultDatabase            = "myanimelist"
	jikanEndpoint              = "https://api.jikan.moe/v4/anime"
)

type jikanClient struct {
	httpClient *http.Client
	limiter    *ratelimit.Controller
}

func newJikanClient() *jikanClient {
	return &jikanClient{
		httpClient: &http.Client{Timeout: 30 * time.Second},
		limiter:    ratelimit.NewController(60),
	}
}

func (c *jikanClient) fetchPage(ctx context.Context, page, perPage int) (jikanResponse, error) {
	var payload jikanResponse

	if perPage <= 0 {
		perPage = 25
	}
	if perPage > 25 {
		perPage = 25
	}

	query := fmt.Sprintf("%s?page=%d&limit=%d&order_by=mal_id&sort=asc", jikanEndpoint, page, perPage)

	for attempt := 0; attempt < 6; attempt++ {
		if err := c.limiter.Wait(ctx); err != nil {
			return payload, err
		}

		req, err := http.NewRequestWithContext(ctx, http.MethodGet, query, nil)
		if err != nil {
			return payload, fmt.Errorf("build request: %w", err)
		}
		req.Header.Set("Accept", "application/json")
		req.Header.Set("User-Agent", "animedb-ingestor/1.0")

		resp, err := c.httpClient.Do(req)
		if err != nil {
			if ctx.Err() != nil {
				return payload, ctx.Err()
			}
			time.Sleep(time.Duration(attempt+1) * 500 * time.Millisecond)
			continue
		}

		sleep := c.limiter.AdjustFromResponse(resp)

		if resp.StatusCode == http.StatusTooManyRequests {
			retry := parseRetryAfter(resp.Header)
			resp.Body.Close()
			if err := sleepContext(ctx, retry); err != nil {
				return payload, err
			}
			continue
		}

		if resp.StatusCode >= 300 {
			defer resp.Body.Close()
			data, _ := io.ReadAll(resp.Body)
			return payload, fmt.Errorf("jikan request failed: status=%d body=%s", resp.StatusCode, truncate(string(data), 512))
		}

		body, err := io.ReadAll(resp.Body)
		resp.Body.Close()
		if err != nil {
			return payload, fmt.Errorf("read response: %w", err)
		}

		if err := json.Unmarshal(body, &payload); err != nil {
			return payload, fmt.Errorf("decode response: %w", err)
		}

		if len(payload.Errors) > 0 {
			return payload, fmt.Errorf("jikan errors: %v", payload.Errors)
		}

		if sleep > 0 {
			if err := sleepContext(ctx, sleep); err != nil {
				return payload, err
			}
		}

		return payload, nil
	}

	return payload, errors.New("exhausted retries fetching Jikan data")
}

type jikanResponse struct {
	Pagination jikanPagination   `json:"pagination"`
	Data       []json.RawMessage `json:"data"`
	Errors     []string          `json:"error"`
}

type jikanPagination struct {
	LastVisiblePage int  `json:"last_visible_page"`
	HasNextPage     bool `json:"has_next_page"`
	CurrentPage     int  `json:"current_page"`
	Items           struct {
		Count   int `json:"count"`
		Total   int `json:"total"`
		PerPage int `json:"per_page"`
	} `json:"items"`
}

type jikanAnime struct {
	Raw             json.RawMessage `json:"-"`
	MalID           int             `json:"mal_id"`
	URL             string          `json:"url"`
	Title           string          `json:"title"`
	TitleEnglish    string          `json:"title_english"`
	TitleJapanese   string          `json:"title_japanese"`
	Type            string          `json:"type"`
	Source          string          `json:"source"`
	Episodes        *int            `json:"episodes"`
	Status          string          `json:"status"`
	Airing          bool            `json:"airing"`
	Aired           airedInfo       `json:"aired"`
	Duration        string          `json:"duration"`
	Rating          string          `json:"rating"`
	Score           *float64        `json:"score"`
	ScoredBy        *int            `json:"scored_by"`
	Rank            *int            `json:"rank"`
	Popularity      *int            `json:"popularity"`
	Members         *int            `json:"members"`
	Favorites       *int            `json:"favorites"`
	Synopsis        string          `json:"synopsis"`
	Background      string          `json:"background"`
	Season          string          `json:"season"`
	Year            *int            `json:"year"`
	Broadcast       json.RawMessage `json:"broadcast"`
	Titles          json.RawMessage `json:"titles"`
	Images          json.RawMessage `json:"images"`
	Trailer         json.RawMessage `json:"trailer"`
	Producers       json.RawMessage `json:"producers"`
	Licensors       json.RawMessage `json:"licensors"`
	Studios         json.RawMessage `json:"studios"`
	Genres          json.RawMessage `json:"genres"`
	Themes          json.RawMessage `json:"themes"`
	Demographics    json.RawMessage `json:"demographics"`
	SeasonInt       *int            `json:"-"`
	PremieredString string          `json:"premiered"`
}

type airedInfo struct {
	From string `json:"from"`
	To   string `json:"to"`
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

	flag.StringVar(&adminDSN, "dsn", defaultMyAnimeListAdminDSN, "Admin Postgres DSN that has privileges to create databases")
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

	if err := postgres.EnsureSchemas(ctx, db, []string{
		`CREATE EXTENSION IF NOT EXISTS unaccent;`,
		`CREATE EXTENSION IF NOT EXISTS pg_trgm;`,
		`CREATE OR REPLACE FUNCTION normalize_title(input TEXT)
RETURNS TEXT
LANGUAGE sql
IMMUTABLE
STRICT
AS $$
	SELECT COALESCE(
		trim(BOTH ' ' FROM regexp_replace(lower(unaccent(input)), '[^a-z0-9]+', ' ', 'g')),
		''
	);
$$;`,
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
		`ALTER TABLE anime
ADD COLUMN IF NOT EXISTS normalized_name TEXT GENERATED ALWAYS AS (
	normalize_title(
		COALESCE(title, '') || ' ' ||
		COALESCE(title_english, '') || ' ' ||
		COALESCE(title_japanese, '')
	)
) STORED;`,
		`CREATE INDEX IF NOT EXISTS anime_year_idx ON anime (year);`,
		`CREATE INDEX IF NOT EXISTS anime_season_idx ON anime (season);`,
		`CREATE INDEX IF NOT EXISTS idx_anime_normalized_name_trgm ON anime USING gin (normalized_name gin_trgm_ops);`,
	}); err != nil {
		log.Fatalf("ensure schema: %v", err)
	}

	client := newJikanClient()

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
		resp, err := client.fetchPage(ctx, page, perPage)
		if err != nil {
			log.Fatalf("fetch page %d: %v", page, err)
		}

		if len(resp.Data) == 0 {
			log.Printf("No more anime returned at page %d, stopping.", page)
			break
		}

		items, err := decodeAnime(resp.Data)
		if err != nil {
			log.Fatalf("decode response data: %v", err)
		}

		inserted, err := persistAnime(ctx, db, items)
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

func decodeAnime(rawItems []json.RawMessage) ([]jikanAnime, error) {
	result := make([]jikanAnime, 0, len(rawItems))
	for _, raw := range rawItems {
		var item jikanAnime
		if err := json.Unmarshal(raw, &item); err != nil {
			return nil, fmt.Errorf("unmarshal anime entry: %w", err)
		}
		item.Raw = raw
		result = append(result, item)
	}
	return result, nil
}

func persistAnime(ctx context.Context, db *sql.DB, items []jikanAnime) (int, error) {
	const upsert = `
INSERT INTO anime (
	mal_id,
	title,
	title_english,
	title_japanese,
	type,
	source,
	episodes,
	status,
	airing,
	aired_from,
	aired_to,
	duration,
	rating,
	score,
	scored_by,
	rank,
	popularity,
	members,
	favorites,
	synopsis,
	background,
	season,
	year,
	broadcast,
	titles,
	images,
	trailer,
	producers,
	licensors,
	studios,
	genres,
	themes,
	demographics,
	raw
) VALUES (
	$1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20,
	$21, $22, $23, $24, $25, $26, $27, $28, $29, $30, $31, $32, $33, $34
)
ON CONFLICT (mal_id) DO UPDATE SET
	title = EXCLUDED.title,
	title_english = EXCLUDED.title_english,
	title_japanese = EXCLUDED.title_japanese,
	type = EXCLUDED.type,
	source = EXCLUDED.source,
	episodes = EXCLUDED.episodes,
	status = EXCLUDED.status,
	airing = EXCLUDED.airing,
	aired_from = EXCLUDED.aired_from,
	aired_to = EXCLUDED.aired_to,
	duration = EXCLUDED.duration,
	rating = EXCLUDED.rating,
	score = EXCLUDED.score,
	scored_by = EXCLUDED.scored_by,
	rank = EXCLUDED.rank,
	popularity = EXCLUDED.popularity,
	members = EXCLUDED.members,
	favorites = EXCLUDED.favorites,
	synopsis = EXCLUDED.synopsis,
	background = EXCLUDED.background,
	season = EXCLUDED.season,
	year = EXCLUDED.year,
	broadcast = EXCLUDED.broadcast,
	titles = EXCLUDED.titles,
	images = EXCLUDED.images,
	trailer = EXCLUDED.trailer,
	producers = EXCLUDED.producers,
	licensors = EXCLUDED.licensors,
	studios = EXCLUDED.studios,
	genres = EXCLUDED.genres,
	themes = EXCLUDED.themes,
	demographics = EXCLUDED.demographics,
	raw = EXCLUDED.raw;
`

	tx, err := db.BeginTx(ctx, &sql.TxOptions{})
	if err != nil {
		return 0, fmt.Errorf("begin transaction: %w", err)
	}
	defer func() {
		_ = tx.Rollback()
	}()

	stmt, err := tx.PrepareContext(ctx, upsert)
	if err != nil {
		return 0, fmt.Errorf("prepare upsert: %w", err)
	}
	defer stmt.Close()

	var rows int
	for _, item := range items {
		select {
		case <-ctx.Done():
			return rows, ctx.Err()
		default:
		}

		airedFrom, err := parseRFC3339(item.Aired.From)
		if err != nil {
			return rows, fmt.Errorf("parse aired.from for %d: %w", item.MalID, err)
		}
		airedTo, err := parseRFC3339(item.Aired.To)
		if err != nil {
			return rows, fmt.Errorf("parse aired.to for %d: %w", item.MalID, err)
		}

		_, err = stmt.ExecContext(
			ctx,
			item.MalID,
			nullIfEmpty(item.Title),
			nullIfEmpty(item.TitleEnglish),
			nullIfEmpty(item.TitleJapanese),
			nullIfEmpty(item.Type),
			nullIfEmpty(item.Source),
			nullIntPointer(item.Episodes),
			nullIfEmpty(item.Status),
			item.Airing,
			airedFrom,
			airedTo,
			nullIfEmpty(item.Duration),
			nullIfEmpty(item.Rating),
			nullFloatPointer(item.Score),
			nullIntPointer(item.ScoredBy),
			nullIntPointer(item.Rank),
			nullIntPointer(item.Popularity),
			nullIntPointer(item.Members),
			nullIntPointer(item.Favorites),
			normalizeDescription(item.Synopsis),
			normalizeDescription(item.Background),
			nullIfEmpty(item.Season),
			nullIntPointer(item.Year),
			emptyJSONIfNil(item.Broadcast),
			emptyJSONIfNil(item.Titles),
			emptyJSONIfNil(item.Images),
			emptyJSONIfNil(item.Trailer),
			emptyJSONIfNil(item.Producers),
			emptyJSONIfNil(item.Licensors),
			emptyJSONIfNil(item.Studios),
			emptyJSONIfNil(item.Genres),
			emptyJSONIfNil(item.Themes),
			emptyJSONIfNil(item.Demographics),
			emptyJSONIfNil(item.Raw),
		)
		if err != nil {
			return rows, fmt.Errorf("upsert anime %d: %w", item.MalID, err)
		}
		rows++
	}

	if err := tx.Commit(); err != nil {
		return rows, fmt.Errorf("commit transaction: %w", err)
	}
	return rows, nil
}

func parseRFC3339(value string) (sql.NullTime, error) {
	value = strings.TrimSpace(value)
	if value == "" || strings.EqualFold(value, "null") {
		return sql.NullTime{}, nil
	}
	t, err := time.Parse(time.RFC3339, value)
	if err != nil {
		return sql.NullTime{}, fmt.Errorf("invalid RFC3339: %w", err)
	}
	return sql.NullTime{Time: t.UTC(), Valid: true}, nil
}

func nullIntPointer(ptr *int) interface{} {
	if ptr == nil {
		return nil
	}
	return *ptr
}

func nullFloatPointer(ptr *float64) interface{} {
	if ptr == nil {
		return nil
	}
	return *ptr
}

func emptyJSONIfNil(raw json.RawMessage) interface{} {
	if len(raw) == 0 {
		return nil
	}
	trimmed := strings.TrimSpace(string(raw))
	if trimmed == "" || strings.EqualFold(trimmed, "null") {
		return nil
	}
	return json.RawMessage([]byte(trimmed))
}

func parseRetryAfter(header http.Header) time.Duration {
	if header == nil {
		return time.Second * 2
	}
	if raw := header.Get("Retry-After"); raw != "" {
		if seconds, err := strconv.Atoi(raw); err == nil {
			if seconds <= 0 {
				return time.Second * 2
			}
			return time.Duration(seconds) * time.Second
		}
	}
	if reset := header.Get("X-RateLimit-Reset"); reset != "" {
		if ts, err := strconv.ParseInt(reset, 10, 64); err == nil {
			wait := time.Until(time.Unix(ts, 0))
			if wait > 0 {
				return wait
			}
		}
	}
	return time.Second * 2
}

func sleepContext(ctx context.Context, d time.Duration) error {
	if d <= 0 {
		return nil
	}
	timer := time.NewTimer(d)
	defer timer.Stop()

	select {
	case <-ctx.Done():
		return ctx.Err()
	case <-timer.C:
		return nil
	}
}

func normalizeDescription(desc string) string {
	trimmed := strings.TrimSpace(desc)
	if trimmed == "" || trimmed == "null" {
		return ""
	}
	return trimmed
}

func nullIfEmpty(value string) interface{} {
	if strings.TrimSpace(value) == "" {
		return nil
	}
	return value
}

func truncate(s string, limit int) string {
	if len(s) <= limit {
		return s
	}
	return s[:limit] + "..."
}
