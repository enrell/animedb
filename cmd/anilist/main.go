package main

import (
	"bytes"
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

	"github.com/lib/pq"
)

const (
	defaultAdminDSN = "postgres://root:root@localhost:5432/root?sslmode=disable"
	defaultDatabase = "anilist"
	anilistEndpoint = "https://graphql.anilist.co"
	graphQLQuery    = `
query ($page: Int, $perPage: Int, $type: MediaType) {
  Page(page: $page, perPage: $perPage) {
    pageInfo {
      total
      perPage
      currentPage
      hasNextPage
    }
    media(type: $type, sort: ID) {
      id
      type
      title {
        romaji
        english
        native
      }
      synonyms
      description
      format
      status
      episodes
      duration
      countryOfOrigin
      source
      season
      seasonYear
      averageScore
      meanScore
      popularity
      favourites
      genres
      tags {
        id
        name
        rank
        isMediaSpoiler
        isGeneralSpoiler
        description
      }
      startDate {
        year
        month
        day
      }
      endDate {
        year
        month
        day
      }
      coverImage {
        large
      }
      bannerImage
      updatedAt
      siteUrl
      isAdult
      isLicensed
      studios {
        edges {
          isMain
        }
        nodes {
          id
          name
          siteUrl
          isAnimationStudio
        }
      }
    }
  }
}
`
)

type pageResponse struct {
	Data struct {
		Page struct {
			PageInfo pageInfo   `json:"pageInfo"`
			Media    []mediaDTO `json:"media"`
		} `json:"Page"`
	} `json:"data"`
	Errors []graphQLError `json:"errors"`
}

type graphQLError struct {
	Message string `json:"message"`
	Status  int    `json:"status"`
}

type pageInfo struct {
	Total       int  `json:"total"`
	PerPage     int  `json:"perPage"`
	CurrentPage int  `json:"currentPage"`
	HasNextPage bool `json:"hasNextPage"`
}

type mediaDTO struct {
	ID              int           `json:"id"`
	Type            string        `json:"type"`
	Title           mediaTitle    `json:"title"`
	Synonyms        []string      `json:"synonyms"`
	Description     string        `json:"description"`
	Format          string        `json:"format"`
	Status          string        `json:"status"`
	Episodes        *int          `json:"episodes"`
	Duration        *int          `json:"duration"`
	CountryOfOrigin string        `json:"countryOfOrigin"`
	Source          string        `json:"source"`
	Season          string        `json:"season"`
	SeasonYear      *int          `json:"seasonYear"`
	AverageScore    *int          `json:"averageScore"`
	MeanScore       *int          `json:"meanScore"`
	Popularity      *int          `json:"popularity"`
	Favourites      *int          `json:"favourites"`
	Genres          []string      `json:"genres"`
	Tags            []mediaTag    `json:"tags"`
	StartDate       fuzzyDate     `json:"startDate"`
	EndDate         fuzzyDate     `json:"endDate"`
	CoverImage      coverImage    `json:"coverImage"`
	BannerImage     string        `json:"bannerImage"`
	UpdatedAt       *int64        `json:"updatedAt"`
	SiteURL         string        `json:"siteUrl"`
	IsAdult         bool          `json:"isAdult"`
	IsLicensed      *bool         `json:"isLicensed"`
	Studios         studioPayload `json:"studios"`
}

type mediaTitle struct {
	Romaji  string `json:"romaji"`
	English string `json:"english"`
	Native  string `json:"native"`
}

type mediaTag struct {
	ID               int    `json:"id"`
	Name             string `json:"name"`
	Rank             *int   `json:"rank"`
	IsMediaSpoiler   bool   `json:"isMediaSpoiler"`
	IsGeneralSpoiler bool   `json:"isGeneralSpoiler"`
	Description      string `json:"description"`
}

type fuzzyDate struct {
	Year  *int `json:"year"`
	Month *int `json:"month"`
	Day   *int `json:"day"`
}

type coverImage struct {
	Large string `json:"large"`
}

type studioPayload struct {
	Edges []struct {
		IsMain bool `json:"isMain"`
	} `json:"edges"`
	Nodes []struct {
		ID                int    `json:"id"`
		Name              string `json:"name"`
		SiteURL           string `json:"siteUrl"`
		IsAnimationStudio bool   `json:"isAnimationStudio"`
	} `json:"nodes"`
}

type aniListClient struct {
	httpClient *http.Client
	limiter    *ratelimit.Controller
}

func newAniListClient() *aniListClient {
	return &aniListClient{
		httpClient: &http.Client{
			Timeout: 30 * time.Second,
		},
		limiter: ratelimit.NewController(90),
	}
}

func (c *aniListClient) fetchPage(ctx context.Context, page, perPage int, mediaType string) (pageResponse, error) {
	var payload pageResponse
	if perPage <= 0 {
		perPage = 50
	}
	if perPage > 50 {
		perPage = 50
	}

	body, err := json.Marshal(map[string]any{
		"query": graphQLQuery,
		"variables": map[string]any{
			"page":    page,
			"perPage": perPage,
			"type":    mediaType,
		},
	})
	if err != nil {
		return payload, fmt.Errorf("encode request: %w", err)
	}

	for attempt := 0; attempt < 6; attempt++ {
		if err := c.limiter.Wait(ctx); err != nil {
			return payload, err
		}

		req, err := http.NewRequestWithContext(ctx, http.MethodPost, anilistEndpoint, bytes.NewBuffer(body))
		if err != nil {
			return payload, fmt.Errorf("build request: %w", err)
		}
		req.Header.Set("Content-Type", "application/json")
		req.Header.Set("Accept", "application/json")

		resp, err := c.httpClient.Do(req)
		if err != nil {
			if ctx.Err() != nil {
				return payload, ctx.Err()
			}
			time.Sleep(time.Duration(attempt+1) * 500 * time.Millisecond)
			continue
		}

		sleepDuration := c.limiter.AdjustFromResponse(resp)

		if resp.StatusCode == http.StatusTooManyRequests {
			retryAfter := parseRetryAfter(resp.Header)
			resp.Body.Close()
			if err := sleepContext(ctx, retryAfter); err != nil {
				return payload, err
			}
			continue
		}

		if resp.StatusCode >= 300 {
			defer resp.Body.Close()
			data, _ := io.ReadAll(resp.Body)
			return payload, fmt.Errorf("anilist request failed: status=%d body=%s", resp.StatusCode, truncate(string(data), 512))
		}

		respBytes, err := io.ReadAll(resp.Body)
		resp.Body.Close()
		if err != nil {
			return payload, fmt.Errorf("read response: %w", err)
		}

		if err := json.Unmarshal(respBytes, &payload); err != nil {
			return payload, fmt.Errorf("decode response: %w", err)
		}
		if len(payload.Errors) > 0 {
			return payload, fmt.Errorf("graphql errors: %v", graphQLErrorMessages(payload.Errors))
		}

		if sleepDuration > 0 {
			if err := sleepContext(ctx, sleepDuration); err != nil {
				return payload, err
			}
		}

		return payload, nil
	}

	return payload, errors.New("exhausted retries fetching AniList data")
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
	}); err != nil {
		log.Fatalf("ensure schema: %v", err)
	}

	client := newAniListClient()

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
		resp, err := client.fetchPage(ctx, page, perPage, mediaType)
		if err != nil {
			log.Fatalf("fetch page %d: %v", page, err)
		}

		if len(resp.Data.Page.Media) == 0 {
			log.Printf("No more media returned at page %d, stopping.", page)
			break
		}

		inserted, err := persistMedia(ctx, db, resp.Data.Page.Media)
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

func persistMedia(ctx context.Context, db *sql.DB, mediaItems []mediaDTO) (int, error) {
	const upsertStatement = `
INSERT INTO media (
	id,
	type,
	title_romaji,
	title_english,
	title_native,
	synonyms,
	description,
	format,
	status,
	episodes,
	duration,
	country_of_origin,
	source,
	season,
	season_year,
	average_score,
	mean_score,
	popularity,
	favourites,
	genres,
	tags,
	studios,
	start_date_year,
	start_date_month,
	start_date_day,
	end_date_year,
	end_date_month,
	end_date_day,
	cover_image,
	banner_image,
	updated_at,
	site_url,
	is_adult,
	is_licensed
) VALUES (
	$1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20,
	$21, $22, $23, $24, $25, $26, $27, $28, $29, $30, $31, $32, $33, $34
)
ON CONFLICT (id) DO UPDATE SET
	type = EXCLUDED.type,
	title_romaji = EXCLUDED.title_romaji,
	title_english = EXCLUDED.title_english,
	title_native = EXCLUDED.title_native,
	synonyms = EXCLUDED.synonyms,
	description = EXCLUDED.description,
	format = EXCLUDED.format,
	status = EXCLUDED.status,
	episodes = EXCLUDED.episodes,
	duration = EXCLUDED.duration,
	country_of_origin = EXCLUDED.country_of_origin,
	source = EXCLUDED.source,
	season = EXCLUDED.season,
	season_year = EXCLUDED.season_year,
	average_score = EXCLUDED.average_score,
	mean_score = EXCLUDED.mean_score,
	popularity = EXCLUDED.popularity,
	favourites = EXCLUDED.favourites,
	genres = EXCLUDED.genres,
	tags = EXCLUDED.tags,
	studios = EXCLUDED.studios,
	start_date_year = EXCLUDED.start_date_year,
	start_date_month = EXCLUDED.start_date_month,
	start_date_day = EXCLUDED.start_date_day,
	end_date_year = EXCLUDED.end_date_year,
	end_date_month = EXCLUDED.end_date_month,
	end_date_day = EXCLUDED.end_date_day,
	cover_image = EXCLUDED.cover_image,
	banner_image = EXCLUDED.banner_image,
	updated_at = EXCLUDED.updated_at,
	site_url = EXCLUDED.site_url,
	is_adult = EXCLUDED.is_adult,
	is_licensed = EXCLUDED.is_licensed;
`

	tx, err := db.BeginTx(ctx, &sql.TxOptions{})
	if err != nil {
		return 0, fmt.Errorf("begin transaction: %w", err)
	}
	defer func() {
		_ = tx.Rollback()
	}()

	stmt, err := tx.PrepareContext(ctx, upsertStatement)
	if err != nil {
		return 0, fmt.Errorf("prepare upsert: %w", err)
	}
	defer stmt.Close()

	var rows int
	for _, m := range mediaItems {
		select {
		case <-ctx.Done():
			return rows, ctx.Err()
		default:
		}

		tagJSON, err := json.Marshal(m.Tags)
		if err != nil {
			return rows, fmt.Errorf("marshal tags for media %d: %w", m.ID, err)
		}
		studioJSON, err := json.Marshal(m.Studios)
		if err != nil {
			return rows, fmt.Errorf("marshal studios for media %d: %w", m.ID, err)
		}
		var updatedAt sql.NullTime
		if m.UpdatedAt != nil && *m.UpdatedAt > 0 {
			updatedAt = sql.NullTime{
				Time:  time.Unix(*m.UpdatedAt, 0).UTC(),
				Valid: true,
			}
		}

		var episodes sql.NullInt64
		if m.Episodes != nil {
			episodes.Int64 = int64(*m.Episodes)
			episodes.Valid = true
		}
		var duration sql.NullInt64
		if m.Duration != nil {
			duration.Int64 = int64(*m.Duration)
			duration.Valid = true
		}
		var seasonYear sql.NullInt64
		if m.SeasonYear != nil {
			seasonYear.Int64 = int64(*m.SeasonYear)
			seasonYear.Valid = true
		}
		var averageScore sql.NullInt64
		if m.AverageScore != nil {
			averageScore.Int64 = int64(*m.AverageScore)
			averageScore.Valid = true
		}
		var meanScore sql.NullInt64
		if m.MeanScore != nil {
			meanScore.Int64 = int64(*m.MeanScore)
			meanScore.Valid = true
		}
		var popularity sql.NullInt64
		if m.Popularity != nil {
			popularity.Int64 = int64(*m.Popularity)
			popularity.Valid = true
		}
		var favourites sql.NullInt64
		if m.Favourites != nil {
			favourites.Int64 = int64(*m.Favourites)
			favourites.Valid = true
		}

		var isLicensed sql.NullBool
		if m.IsLicensed != nil {
			isLicensed.Bool = *m.IsLicensed
			isLicensed.Valid = true
		}

		_, err = stmt.ExecContext(
			ctx,
			m.ID,
			m.Type,
			nullIfEmpty(m.Title.Romaji),
			nullIfEmpty(m.Title.English),
			nullIfEmpty(m.Title.Native),
			pq.Array(m.Synonyms),
			normalizeDescription(m.Description),
			nullIfEmpty(m.Format),
			nullIfEmpty(m.Status),
			episodes,
			duration,
			nullIfEmpty(m.CountryOfOrigin),
			nullIfEmpty(m.Source),
			nullIfEmpty(m.Season),
			seasonYear,
			averageScore,
			meanScore,
			popularity,
			favourites,
			pq.Array(m.Genres),
			tagJSON,
			studioJSON,
			valueOrZero(m.StartDate.Year),
			valueOrZero(m.StartDate.Month),
			valueOrZero(m.StartDate.Day),
			valueOrZero(m.EndDate.Year),
			valueOrZero(m.EndDate.Month),
			valueOrZero(m.EndDate.Day),
			nullIfEmpty(m.CoverImage.Large),
			nullIfEmpty(m.BannerImage),
			updatedAt,
			nullIfEmpty(m.SiteURL),
			m.IsAdult,
			isLicensed,
		)
		if err != nil {
			return rows, fmt.Errorf("upsert media %d: %w", m.ID, err)
		}
		rows++
	}

	if err := tx.Commit(); err != nil {
		return rows, fmt.Errorf("commit transaction: %w", err)
	}
	return rows, nil
}

func valueOrZero(ptr *int) any {
	if ptr == nil || *ptr == 0 {
		return nil
	}
	return *ptr
}

func nullIfEmpty(s string) any {
	if strings.TrimSpace(s) == "" {
		return nil
	}
	return s
}

func normalizeDescription(desc string) string {
	trimmed := strings.TrimSpace(desc)
	if trimmed == "" || trimmed == "null" {
		return ""
	}
	return trimmed
}

func graphQLErrorMessages(errs []graphQLError) []string {
	out := make([]string, 0, len(errs))
	for _, e := range errs {
		if e.Status != 0 {
			out = append(out, fmt.Sprintf("%s (status %d)", e.Message, e.Status))
		} else {
			out = append(out, e.Message)
		}
	}
	return out
}

func parseRetryAfter(header http.Header) time.Duration {
	if header == nil {
		return time.Minute
	}
	if raw := header.Get("Retry-After"); raw != "" {
		if seconds, err := strconv.Atoi(raw); err == nil {
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
	// default to one minute fallback
	return time.Minute
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

func truncate(s string, limit int) string {
	if len(s) <= limit {
		return s
	}
	return s[:limit] + "..."
}
