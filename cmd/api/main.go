package main

import (
	"context"
	"database/sql"
	"encoding/json"
	"errors"
	"flag"
	"fmt"
	"log"
	"net/http"
	"strconv"
	"strings"
	"time"

	"animedb/internal/postgres"

	"github.com/go-chi/chi/v5"
	"github.com/go-chi/chi/v5/middleware"
	"github.com/lib/pq"
)

const (
	defaultListenAddr          = ":8081"
	defaultAniListDSN          = "postgres://root:root@localhost:5432/anilist?sslmode=disable"
	defaultMyAnimeListDSN      = "postgres://root:root@localhost:5432/myanimelist?sslmode=disable"
	queryTimeout               = 15 * time.Second
	trigramSimilarityThreshold = 0.30
	normalizeTitleFunctionSQL  = `
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
)

func main() {
	var (
		listenAddr     string
		adminDSN       string
		anilistDSN     string
		myAnimeListDSN string
	)

	flag.StringVar(&listenAddr, "listen", defaultListenAddr, "HTTP listen address")
	flag.StringVar(&adminDSN, "admin-dsn", "postgres://root:root@localhost:5432/root?sslmode=disable", "Admin Postgres DSN used to ensure target databases exist")
	flag.StringVar(&anilistDSN, "anilist-dsn", defaultAniListDSN, "Postgres DSN for the AniList database")
	flag.StringVar(&myAnimeListDSN, "myanimelist-dsn", defaultMyAnimeListDSN, "Postgres DSN for the MyAnimeList database")
	flag.Parse()

	args := flag.Args()
	if len(args) > 0 {
		listenAddr = args[0]
	}
	if len(args) > 1 {
		anilistDSN = args[1]
	}
	if len(args) > 2 {
		myAnimeListDSN = args[2]
	}

	ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
	defer cancel()

	aniTargetDSN, err := ensureDSN(ctx, adminDSN, anilistDSN, "anilist")
	if err != nil {
		log.Fatalf("prepare AniList database: %v", err)
	}

	malTargetDSN, err := ensureDSN(ctx, adminDSN, myAnimeListDSN, "myanimelist")
	if err != nil {
		log.Fatalf("prepare MyAnimeList database: %v", err)
	}

	aniDB, err := openAndPing(aniTargetDSN)
	if err != nil {
		log.Fatalf("connect AniList database: %v", err)
	}
	defer aniDB.Close()
	if err := ensureAniListSearchHelpers(context.Background(), aniDB); err != nil {
		log.Fatalf("ensure AniList search helpers: %v", err)
	}

	malDB, err := openAndPing(malTargetDSN)
	if err != nil {
		log.Fatalf("connect MyAnimeList database: %v", err)
	}
	defer malDB.Close()
	if err := ensureMyAnimeListSearchHelpers(context.Background(), malDB); err != nil {
		log.Fatalf("ensure MyAnimeList search helpers: %v", err)
	}

	srv := &server{
		anilistDB:     aniDB,
		myAnimeListDB: malDB,
	}

	router := srv.routes()

	httpServer := &http.Server{
		Addr:              listenAddr,
		Handler:           router,
		ReadHeaderTimeout: 10 * time.Second,
	}

	log.Printf("REST API listening on %s", listenAddr)
	if err := httpServer.ListenAndServe(); err != nil && !errors.Is(err, http.ErrServerClosed) {
		log.Fatalf("http server error: %v", err)
	}
}

func ensureDSN(ctx context.Context, adminDSN, providedDSN, database string) (string, error) {
	if strings.Contains(providedDSN, fmt.Sprintf("/%s", database)) || strings.Contains(providedDSN, fmt.Sprintf("dbname=%s", database)) {
		// Ensure the database exists before using the provided DSN.
		if _, err := postgres.EnsureDatabase(ctx, adminDSN, database); err != nil {
			return "", err
		}
		return providedDSN, nil
	}
	return postgres.EnsureDatabase(ctx, adminDSN, database)
}

func openAndPing(dsn string) (*sql.DB, error) {
	db, err := sql.Open("postgres", dsn)
	if err != nil {
		return nil, err
	}
	ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
	defer cancel()
	if err := db.PingContext(ctx); err != nil {
		db.Close()
		return nil, err
	}
	return db, nil
}

func ensureAniListSearchHelpers(ctx context.Context, db *sql.DB) error {
	schemaCtx, cancel := context.WithTimeout(ctx, 5*time.Minute)
	defer cancel()
	statements := []string{
		`CREATE EXTENSION IF NOT EXISTS unaccent;`,
		`CREATE EXTENSION IF NOT EXISTS pg_trgm;`,
		normalizeTitleFunctionSQL,
	}
	exists, err := tableExists(schemaCtx, db, "media")
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
	return postgres.EnsureSchemas(schemaCtx, db, statements)
}

func ensureMyAnimeListSearchHelpers(ctx context.Context, db *sql.DB) error {
	schemaCtx, cancel := context.WithTimeout(ctx, 5*time.Minute)
	defer cancel()
	statements := []string{
		`CREATE EXTENSION IF NOT EXISTS unaccent;`,
		`CREATE EXTENSION IF NOT EXISTS pg_trgm;`,
		normalizeTitleFunctionSQL,
	}
	exists, err := tableExists(schemaCtx, db, "anime")
	if err != nil {
		return err
	}
	if exists {
		statements = append(statements,
			`ALTER TABLE IF EXISTS anime
	ADD COLUMN IF NOT EXISTS normalized_name TEXT GENERATED ALWAYS AS (
		normalize_title(
			COALESCE(title, '') || ' ' ||
			COALESCE(title_english, '') || ' ' ||
			COALESCE(title_japanese, '')
		)
	) STORED;`,
			`CREATE INDEX IF NOT EXISTS idx_anime_normalized_name_trgm ON anime USING gin (normalized_name gin_trgm_ops);`,
		)
	}
	return postgres.EnsureSchemas(schemaCtx, db, statements)
}

func tableExists(ctx context.Context, db *sql.DB, table string) (bool, error) {
	const q = `
SELECT EXISTS (
	SELECT 1
	FROM information_schema.tables
	WHERE table_schema = current_schema()
		AND table_name = $1
);`
	var exists bool
	if err := db.QueryRowContext(ctx, q, table).Scan(&exists); err != nil {
		return false, err
	}
	return exists, nil
}

type server struct {
	anilistDB     *sql.DB
	myAnimeListDB *sql.DB
}

func (s *server) routes() http.Handler {
	r := chi.NewRouter()
	r.Use(middleware.RequestID)
	r.Use(middleware.RealIP)
	r.Use(middleware.Recoverer)

	r.Get("/healthz", func(w http.ResponseWriter, r *http.Request) {
		writeJSON(w, http.StatusOK, map[string]string{"status": "ok"})
	})

	r.Route("/anilist", func(r chi.Router) {
		r.Get("/media/search", s.handleAniListMediaSearch)
		r.Get("/media", s.handleAniListMediaList)
		r.Get("/media/{id}", s.handleAniListMediaGet)
	})

	r.Route("/myanimelist", func(r chi.Router) {
		r.Get("/anime/search", s.handleMyAnimeListSearch)
		r.Get("/anime", s.handleMyAnimeListList)
		r.Get("/anime/{id}", s.handleMyAnimeListGet)
	})

	return r
}

type paginationMeta struct {
	Page     int `json:"page"`
	PageSize int `json:"page_size"`
	Total    int `json:"total"`
}

type listResponse[T any] struct {
	Data       []T            `json:"data"`
	Pagination paginationMeta `json:"pagination"`
}

type searchResult struct {
	ID      int     `json:"id"`
	Title   string  `json:"title,omitempty"`
	Romaji  string  `json:"romaji,omitempty"`
	English string  `json:"english,omitempty"`
	Native  string  `json:"native,omitempty"`
	Score   float64 `json:"score"`
}

// AniList handlers and models.

func (s *server) handleAniListMediaList(w http.ResponseWriter, r *http.Request) {
	ctx, cancel := context.WithTimeout(r.Context(), queryTimeout)
	defer cancel()

	page, pageSize := parsePagination(r.URL.Query().Get("page"), r.URL.Query().Get("page_size"), 20, 100)
	offset := (page - 1) * pageSize

	var (
		args       []any
		conditions []string
	)

	var searchArgPos int
	if search := strings.TrimSpace(r.URL.Query().Get("search")); search != "" {
		idx := len(args) + 1
		conditions = append(conditions, fmt.Sprintf(
			"((length(normalize_title($%d)) < 3 AND normalized_title ILIKE '%%' || normalize_title($%d) || '%%') OR similarity(normalized_title, normalize_title($%d)) >= %.2f)",
			idx, idx, idx, trigramSimilarityThreshold))
		args = append(args, search)
		searchArgPos = idx
	}
	if titleRomaji := strings.TrimSpace(r.URL.Query().Get("title_romaji")); titleRomaji != "" {
		idx := len(args) + 1
		conditions = append(conditions, fmt.Sprintf("title_romaji ILIKE $%d", idx))
		args = append(args, "%"+titleRomaji+"%")
	}
	if titleEnglish := strings.TrimSpace(r.URL.Query().Get("title_english")); titleEnglish != "" {
		idx := len(args) + 1
		conditions = append(conditions, fmt.Sprintf("title_english ILIKE $%d", idx))
		args = append(args, "%"+titleEnglish+"%")
	}
	if titleNative := strings.TrimSpace(r.URL.Query().Get("title_native")); titleNative != "" {
		idx := len(args) + 1
		conditions = append(conditions, fmt.Sprintf("title_native ILIKE $%d", idx))
		args = append(args, "%"+titleNative+"%")
	}

	if mediaType := strings.TrimSpace(r.URL.Query().Get("type")); mediaType != "" {
		idx := len(args) + 1
		conditions = append(conditions, fmt.Sprintf("type = $%d", idx))
		args = append(args, mediaType)
	}

	if season := strings.TrimSpace(r.URL.Query().Get("season")); season != "" {
		idx := len(args) + 1
		conditions = append(conditions, fmt.Sprintf("season = $%d", idx))
		args = append(args, strings.ToUpper(season))
	}

	if yearStr := strings.TrimSpace(r.URL.Query().Get("season_year")); yearStr != "" {
		if year, err := strconv.Atoi(yearStr); err == nil {
			idx := len(args) + 1
			conditions = append(conditions, fmt.Sprintf("season_year = $%d", idx))
			args = append(args, year)
		}
	}

	whereClause := buildWhereClause(conditions)

	total, err := s.countAniList(ctx, whereClause, args)
	if err != nil {
		writeError(w, http.StatusInternalServerError, err)
		return
	}

	query := `
SELECT
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
FROM media
`
	queryArgs := append([]any{}, args...)
	if whereClause != "" {
		query += " " + whereClause
	}
	orderClause := " ORDER BY id"
	if searchArgPos > 0 {
		orderClause = fmt.Sprintf(" ORDER BY similarity(normalized_title, normalize_title($%d)) DESC, id", searchArgPos)
	}
	query += orderClause
	query += fmt.Sprintf(" LIMIT $%d OFFSET $%d", len(queryArgs)+1, len(queryArgs)+2)
	queryArgs = append(queryArgs, pageSize, offset)

	rows, err := s.anilistDB.QueryContext(ctx, query, queryArgs...)
	if err != nil {
		writeError(w, http.StatusInternalServerError, err)
		return
	}
	defer rows.Close()

	var results []aniListMedia
	for rows.Next() {
		item, err := scanAniList(rows)
		if err != nil {
			writeError(w, http.StatusInternalServerError, err)
			return
		}
		results = append(results, item)
	}
	if err := rows.Err(); err != nil {
		writeError(w, http.StatusInternalServerError, err)
		return
	}

	writeJSON(w, http.StatusOK, listResponse[aniListMedia]{
		Data: results,
		Pagination: paginationMeta{
			Page:     page,
			PageSize: pageSize,
			Total:    total,
		},
	})
}

func (s *server) handleAniListMediaGet(w http.ResponseWriter, r *http.Request) {
	ctx, cancel := context.WithTimeout(r.Context(), queryTimeout)
	defer cancel()

	idStr := chi.URLParam(r, "id")
	id, err := strconv.Atoi(idStr)
	if err != nil || id <= 0 {
		writeError(w, http.StatusBadRequest, fmt.Errorf("invalid id: %s", idStr))
		return
	}

	query := `
SELECT
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
FROM media
WHERE id = $1
`

	row := s.anilistDB.QueryRowContext(ctx, query, id)
	item, err := scanAniList(row)
	if err != nil {
		if errors.Is(err, sql.ErrNoRows) {
			writeError(w, http.StatusNotFound, fmt.Errorf("media %d not found", id))
		} else {
			writeError(w, http.StatusInternalServerError, err)
		}
		return
	}

	writeJSON(w, http.StatusOK, item)
}

func (s *server) handleAniListMediaSearch(w http.ResponseWriter, r *http.Request) {
	ctx, cancel := context.WithTimeout(r.Context(), queryTimeout)
	defer cancel()

	search := strings.TrimSpace(r.URL.Query().Get("search"))
	if search == "" {
		writeError(w, http.StatusBadRequest, fmt.Errorf("search query required"))
		return
	}

	const query = `
SELECT
	id,
	title_romaji,
	title_english,
	title_native,
	similarity(normalized_title, normalize_title($1)) AS score
FROM media
WHERE (
	length(normalize_title($1)) < 3
		AND normalized_title ILIKE '%' || normalize_title($1) || '%'
) OR normalized_title % normalize_title($1)
ORDER BY score DESC, id
LIMIT 5;
`

	rows, err := s.anilistDB.QueryContext(ctx, query, search)
	if err != nil {
		writeError(w, http.StatusInternalServerError, err)
		return
	}
	defer rows.Close()

	var results []searchResult
	for rows.Next() {
		var (
			id           int
			titleRomaji  sql.NullString
			titleEnglish sql.NullString
			titleNative  sql.NullString
			score        sql.NullFloat64
		)
		if err := rows.Scan(&id, &titleRomaji, &titleEnglish, &titleNative, &score); err != nil {
			writeError(w, http.StatusInternalServerError, err)
			return
		}
		if !score.Valid {
			continue
		}

		result := searchResult{
			ID:      id,
			Romaji:  titleRomaji.String,
			English: titleEnglish.String,
			Native:  titleNative.String,
			Score:   score.Float64,
		}
		result.Title = firstNonEmpty(titleEnglish.String, titleRomaji.String, titleNative.String)
		results = append(results, result)
	}
	if err := rows.Err(); err != nil {
		writeError(w, http.StatusInternalServerError, err)
		return
	}

	writeJSON(w, http.StatusOK, results)
}

func (s *server) countAniList(ctx context.Context, where string, args []any) (int, error) {
	query := "SELECT COUNT(*) FROM media"
	if where != "" {
		query += " " + where
	}
	var total int
	if err := s.anilistDB.QueryRowContext(ctx, query, args...).Scan(&total); err != nil {
		return 0, err
	}
	return total, nil
}

type aniListMedia struct {
	ID              int             `json:"id"`
	Type            string          `json:"type,omitempty"`
	Title           aniListTitle    `json:"title"`
	Synonyms        []string        `json:"synonyms,omitempty"`
	Description     string          `json:"description,omitempty"`
	Format          string          `json:"format,omitempty"`
	Status          string          `json:"status,omitempty"`
	Episodes        *int            `json:"episodes,omitempty"`
	Duration        *int            `json:"duration,omitempty"`
	CountryOfOrigin string          `json:"country_of_origin,omitempty"`
	Source          string          `json:"source,omitempty"`
	Season          string          `json:"season,omitempty"`
	SeasonYear      *int            `json:"season_year,omitempty"`
	AverageScore    *int            `json:"average_score,omitempty"`
	MeanScore       *int            `json:"mean_score,omitempty"`
	Popularity      *int            `json:"popularity,omitempty"`
	Favourites      *int            `json:"favourites,omitempty"`
	Genres          []string        `json:"genres,omitempty"`
	Tags            json.RawMessage `json:"tags,omitempty"`
	Studios         json.RawMessage `json:"studios,omitempty"`
	StartDate       partialDate     `json:"start_date,omitempty"`
	EndDate         partialDate     `json:"end_date,omitempty"`
	CoverImage      string          `json:"cover_image,omitempty"`
	BannerImage     string          `json:"banner_image,omitempty"`
	UpdatedAt       *time.Time      `json:"updated_at,omitempty"`
	SiteURL         string          `json:"site_url,omitempty"`
	IsAdult         bool            `json:"is_adult"`
	IsLicensed      *bool           `json:"is_licensed,omitempty"`
}

type aniListTitle struct {
	Romaji  string `json:"romaji,omitempty"`
	English string `json:"english,omitempty"`
	Native  string `json:"native,omitempty"`
}

type partialDate struct {
	Year  *int `json:"year,omitempty"`
	Month *int `json:"month,omitempty"`
	Day   *int `json:"day,omitempty"`
}

type rowScanner interface {
	Scan(dest ...any) error
}

func scanAniList(s rowScanner) (aniListMedia, error) {
	var (
		row          aniListMedia
		titleRomaji  sql.NullString
		titleEnglish sql.NullString
		titleNative  sql.NullString
		description  sql.NullString
		format       sql.NullString
		status       sql.NullString
		country      sql.NullString
		source       sql.NullString
		season       sql.NullString
		seasonYear   sql.NullInt64
		averageScore sql.NullInt64
		meanScore    sql.NullInt64
		popularity   sql.NullInt64
		favourites   sql.NullInt64
		tagsRaw      []byte
		studiosRaw   []byte
		startYear    sql.NullInt64
		startMonth   sql.NullInt64
		startDay     sql.NullInt64
		endYear      sql.NullInt64
		endMonth     sql.NullInt64
		endDay       sql.NullInt64
		coverImage   sql.NullString
		bannerImage  sql.NullString
		updatedAt    sql.NullTime
		siteURL      sql.NullString
		isLicensed   sql.NullBool
		episodes     sql.NullInt64
		duration     sql.NullInt64
	)

	var synonyms pq.StringArray
	var genres pq.StringArray

	if err := s.Scan(
		&row.ID,
		&row.Type,
		&titleRomaji,
		&titleEnglish,
		&titleNative,
		&synonyms,
		&description,
		&format,
		&status,
		&episodes,
		&duration,
		&country,
		&source,
		&season,
		&seasonYear,
		&averageScore,
		&meanScore,
		&popularity,
		&favourites,
		&genres,
		&tagsRaw,
		&studiosRaw,
		&startYear,
		&startMonth,
		&startDay,
		&endYear,
		&endMonth,
		&endDay,
		&coverImage,
		&bannerImage,
		&updatedAt,
		&siteURL,
		&row.IsAdult,
		&isLicensed,
	); err != nil {
		return aniListMedia{}, err
	}

	row.Title = aniListTitle{
		Romaji:  titleRomaji.String,
		English: titleEnglish.String,
		Native:  titleNative.String,
	}
	row.Synonyms = copyStringArray([]string(synonyms))
	row.Description = description.String
	row.Format = format.String
	row.Status = status.String
	if episodes.Valid {
		v := int(episodes.Int64)
		row.Episodes = &v
	}
	if duration.Valid {
		v := int(duration.Int64)
		row.Duration = &v
	}
	row.CountryOfOrigin = country.String
	row.Source = source.String
	row.Season = season.String
	if seasonYear.Valid {
		v := int(seasonYear.Int64)
		row.SeasonYear = &v
	}
	if averageScore.Valid {
		v := int(averageScore.Int64)
		row.AverageScore = &v
	}
	if meanScore.Valid {
		v := int(meanScore.Int64)
		row.MeanScore = &v
	}
	if popularity.Valid {
		v := int(popularity.Int64)
		row.Popularity = &v
	}
	if favourites.Valid {
		v := int(favourites.Int64)
		row.Favourites = &v
	}
	row.Genres = copyStringArray([]string(genres))
	if len(tagsRaw) > 0 {
		row.Tags = json.RawMessage(tagsRaw)
	}
	if len(studiosRaw) > 0 {
		row.Studios = json.RawMessage(studiosRaw)
	}
	row.StartDate = partialDate{
		Year:  nullableInt(startYear),
		Month: nullableInt(startMonth),
		Day:   nullableInt(startDay),
	}
	row.EndDate = partialDate{
		Year:  nullableInt(endYear),
		Month: nullableInt(endMonth),
		Day:   nullableInt(endDay),
	}
	row.CoverImage = coverImage.String
	row.BannerImage = bannerImage.String
	if updatedAt.Valid {
		t := updatedAt.Time.UTC()
		row.UpdatedAt = &t
	}
	row.SiteURL = siteURL.String
	if isLicensed.Valid {
		v := isLicensed.Bool
		row.IsLicensed = &v
	}

	return row, nil
}

// MyAnimeList handlers and models.

func (s *server) handleMyAnimeListList(w http.ResponseWriter, r *http.Request) {
	ctx, cancel := context.WithTimeout(r.Context(), queryTimeout)
	defer cancel()

	page, pageSize := parsePagination(r.URL.Query().Get("page"), r.URL.Query().Get("page_size"), 20, 100)
	offset := (page - 1) * pageSize

	var (
		args       []any
		conditions []string
	)

	var malSearchArgPos int
	if search := strings.TrimSpace(r.URL.Query().Get("search")); search != "" {
		idx := len(args) + 1
		conditions = append(conditions, fmt.Sprintf(
			"((length(normalize_title($%d)) < 3 AND normalized_name ILIKE '%%' || normalize_title($%d) || '%%') OR similarity(normalized_name, normalize_title($%d)) >= %.2f)",
			idx, idx, idx, trigramSimilarityThreshold))
		args = append(args, search)
		malSearchArgPos = idx
	}

	if animeType := strings.TrimSpace(r.URL.Query().Get("type")); animeType != "" {
		idx := len(args) + 1
		conditions = append(conditions, fmt.Sprintf("type = $%d", idx))
		args = append(args, animeType)
	}

	if season := strings.TrimSpace(r.URL.Query().Get("season")); season != "" {
		idx := len(args) + 1
		conditions = append(conditions, fmt.Sprintf("season = $%d", idx))
		args = append(args, strings.ToLower(season))
	}

	if yearStr := strings.TrimSpace(r.URL.Query().Get("year")); yearStr != "" {
		if year, err := strconv.Atoi(yearStr); err == nil {
			idx := len(args) + 1
			conditions = append(conditions, fmt.Sprintf("year = $%d", idx))
			args = append(args, year)
		}
	}

	whereClause := buildWhereClause(conditions)

	total, err := s.countMyAnimeList(ctx, whereClause, args)
	if err != nil {
		writeError(w, http.StatusInternalServerError, err)
		return
	}

	query := `
SELECT
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
FROM anime
`

	queryArgs := append([]any{}, args...)
	if whereClause != "" {
		query += " " + whereClause
	}
	orderClause := " ORDER BY mal_id"
	if malSearchArgPos > 0 {
		orderClause = fmt.Sprintf(" ORDER BY similarity(normalized_name, normalize_title($%d)) DESC, mal_id", malSearchArgPos)
	}
	query += orderClause
	query += fmt.Sprintf(" LIMIT $%d OFFSET $%d", len(queryArgs)+1, len(queryArgs)+2)
	queryArgs = append(queryArgs, pageSize, offset)

	rows, err := s.myAnimeListDB.QueryContext(ctx, query, queryArgs...)
	if err != nil {
		writeError(w, http.StatusInternalServerError, err)
		return
	}
	defer rows.Close()

	var results []myAnimeListAnime
	for rows.Next() {
		item, err := scanMyAnimeList(rows)
		if err != nil {
			writeError(w, http.StatusInternalServerError, err)
			return
		}
		results = append(results, item)
	}
	if err := rows.Err(); err != nil {
		writeError(w, http.StatusInternalServerError, err)
		return
	}

	writeJSON(w, http.StatusOK, listResponse[myAnimeListAnime]{
		Data: results,
		Pagination: paginationMeta{
			Page:     page,
			PageSize: pageSize,
			Total:    total,
		},
	})
}

func (s *server) handleMyAnimeListGet(w http.ResponseWriter, r *http.Request) {
	ctx, cancel := context.WithTimeout(r.Context(), queryTimeout)
	defer cancel()

	idStr := chi.URLParam(r, "id")
	id, err := strconv.Atoi(idStr)
	if err != nil || id <= 0 {
		writeError(w, http.StatusBadRequest, fmt.Errorf("invalid id: %s", idStr))
		return
	}

	query := `
SELECT
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
FROM anime
WHERE mal_id = $1
`

	row := s.myAnimeListDB.QueryRowContext(ctx, query, id)
	item, err := scanMyAnimeList(row)
	if err != nil {
		if errors.Is(err, sql.ErrNoRows) {
			writeError(w, http.StatusNotFound, fmt.Errorf("anime %d not found", id))
		} else {
			writeError(w, http.StatusInternalServerError, err)
		}
		return
	}

	writeJSON(w, http.StatusOK, item)
}

func (s *server) handleMyAnimeListSearch(w http.ResponseWriter, r *http.Request) {
	ctx, cancel := context.WithTimeout(r.Context(), queryTimeout)
	defer cancel()

	search := strings.TrimSpace(r.URL.Query().Get("search"))
	if search == "" {
		writeError(w, http.StatusBadRequest, fmt.Errorf("search query required"))
		return
	}

	const query = `
SELECT
	mal_id,
	title,
	title_english,
	title_japanese,
	similarity(normalized_name, normalize_title($1)) AS score
FROM anime
WHERE (
	length(normalize_title($1)) < 3
		AND normalized_name ILIKE '%' || normalize_title($1) || '%'
) OR normalized_name % normalize_title($1)
ORDER BY score DESC, mal_id
LIMIT 5;
`

	rows, err := s.myAnimeListDB.QueryContext(ctx, query, search)
	if err != nil {
		writeError(w, http.StatusInternalServerError, err)
		return
	}
	defer rows.Close()

	var results []searchResult
	for rows.Next() {
		var (
			id            int
			title         sql.NullString
			titleEnglish  sql.NullString
			titleJapanese sql.NullString
			score         sql.NullFloat64
		)
		if err := rows.Scan(&id, &title, &titleEnglish, &titleJapanese, &score); err != nil {
			writeError(w, http.StatusInternalServerError, err)
			return
		}
		if !score.Valid {
			continue
		}

		result := searchResult{
			ID:      id,
			Title:   firstNonEmpty(title.String, titleEnglish.String, titleJapanese.String),
			English: titleEnglish.String,
			Native:  titleJapanese.String,
			Score:   score.Float64,
		}
		results = append(results, result)
	}
	if err := rows.Err(); err != nil {
		writeError(w, http.StatusInternalServerError, err)
		return
	}

	writeJSON(w, http.StatusOK, results)
}

func (s *server) countMyAnimeList(ctx context.Context, where string, args []any) (int, error) {
	query := "SELECT COUNT(*) FROM anime"
	if where != "" {
		query += " " + where
	}
	var total int
	if err := s.myAnimeListDB.QueryRowContext(ctx, query, args...).Scan(&total); err != nil {
		return 0, err
	}
	return total, nil
}

type myAnimeListAnime struct {
	ID            int             `json:"mal_id"`
	Title         string          `json:"title,omitempty"`
	TitleEnglish  string          `json:"title_english,omitempty"`
	TitleJapanese string          `json:"title_japanese,omitempty"`
	Type          string          `json:"type,omitempty"`
	Source        string          `json:"source,omitempty"`
	Episodes      *int            `json:"episodes,omitempty"`
	Status        string          `json:"status,omitempty"`
	Airing        bool            `json:"airing"`
	AiredFrom     *time.Time      `json:"aired_from,omitempty"`
	AiredTo       *time.Time      `json:"aired_to,omitempty"`
	Duration      string          `json:"duration,omitempty"`
	Rating        string          `json:"rating,omitempty"`
	Score         *float64        `json:"score,omitempty"`
	ScoredBy      *int            `json:"scored_by,omitempty"`
	Rank          *int            `json:"rank,omitempty"`
	Popularity    *int            `json:"popularity,omitempty"`
	Members       *int            `json:"members,omitempty"`
	Favorites     *int            `json:"favorites,omitempty"`
	Synopsis      string          `json:"synopsis,omitempty"`
	Background    string          `json:"background,omitempty"`
	Season        string          `json:"season,omitempty"`
	Year          *int            `json:"year,omitempty"`
	Broadcast     json.RawMessage `json:"broadcast,omitempty"`
	Titles        json.RawMessage `json:"titles,omitempty"`
	Images        json.RawMessage `json:"images,omitempty"`
	Trailer       json.RawMessage `json:"trailer,omitempty"`
	Producers     json.RawMessage `json:"producers,omitempty"`
	Licensors     json.RawMessage `json:"licensors,omitempty"`
	Studios       json.RawMessage `json:"studios,omitempty"`
	Genres        json.RawMessage `json:"genres,omitempty"`
	Themes        json.RawMessage `json:"themes,omitempty"`
	Demographics  json.RawMessage `json:"demographics,omitempty"`
	Raw           json.RawMessage `json:"raw,omitempty"`
}

func scanMyAnimeList(s rowScanner) (myAnimeListAnime, error) {
	var (
		row           myAnimeListAnime
		title         sql.NullString
		titleEnglish  sql.NullString
		titleJapanese sql.NullString
		animeType     sql.NullString
		source        sql.NullString
		episodes      sql.NullInt64
		status        sql.NullString
		duration      sql.NullString
		rating        sql.NullString
		score         sql.NullFloat64
		scoredBy      sql.NullInt64
		rank          sql.NullInt64
		popularity    sql.NullInt64
		members       sql.NullInt64
		favorites     sql.NullInt64
		synopsis      sql.NullString
		background    sql.NullString
		season        sql.NullString
		year          sql.NullInt64
		broadcast     []byte
		titles        []byte
		images        []byte
		trailer       []byte
		producers     []byte
		licensors     []byte
		studios       []byte
		genres        []byte
		themes        []byte
		demographics  []byte
		raw           []byte
		airedFrom     sql.NullTime
		airedTo       sql.NullTime
	)

	if err := s.Scan(
		&row.ID,
		&title,
		&titleEnglish,
		&titleJapanese,
		&animeType,
		&source,
		&episodes,
		&status,
		&row.Airing,
		&airedFrom,
		&airedTo,
		&duration,
		&rating,
		&score,
		&scoredBy,
		&rank,
		&popularity,
		&members,
		&favorites,
		&synopsis,
		&background,
		&season,
		&year,
		&broadcast,
		&titles,
		&images,
		&trailer,
		&producers,
		&licensors,
		&studios,
		&genres,
		&themes,
		&demographics,
		&raw,
	); err != nil {
		return myAnimeListAnime{}, err
	}

	row.Title = title.String
	row.TitleEnglish = titleEnglish.String
	row.TitleJapanese = titleJapanese.String
	row.Type = animeType.String
	row.Source = source.String
	if episodes.Valid {
		v := int(episodes.Int64)
		row.Episodes = &v
	}
	row.Status = status.String
	row.Duration = duration.String
	row.Rating = rating.String
	if score.Valid {
		v := score.Float64
		row.Score = &v
	}
	if scoredBy.Valid {
		v := int(scoredBy.Int64)
		row.ScoredBy = &v
	}
	if rank.Valid {
		v := int(rank.Int64)
		row.Rank = &v
	}
	if popularity.Valid {
		v := int(popularity.Int64)
		row.Popularity = &v
	}
	if members.Valid {
		v := int(members.Int64)
		row.Members = &v
	}
	if favorites.Valid {
		v := int(favorites.Int64)
		row.Favorites = &v
	}
	row.Synopsis = synopsis.String
	row.Background = background.String
	row.Season = season.String
	if year.Valid {
		v := int(year.Int64)
		row.Year = &v
	}
	if airedFrom.Valid {
		t := airedFrom.Time.UTC()
		row.AiredFrom = &t
	}
	if airedTo.Valid {
		t := airedTo.Time.UTC()
		row.AiredTo = &t
	}
	row.Broadcast = rawJSON(broadcast)
	row.Titles = rawJSON(titles)
	row.Images = rawJSON(images)
	row.Trailer = rawJSON(trailer)
	row.Producers = rawJSON(producers)
	row.Licensors = rawJSON(licensors)
	row.Studios = rawJSON(studios)
	row.Genres = rawJSON(genres)
	row.Themes = rawJSON(themes)
	row.Demographics = rawJSON(demographics)
	row.Raw = rawJSON(raw)

	return row, nil
}

// Helpers.

func parsePagination(pageStr, sizeStr string, defaultSize, maxSize int) (int, int) {
	page := 1
	if p, err := strconv.Atoi(pageStr); err == nil && p > 0 {
		page = p
	}

	pageSize := defaultSize
	if s, err := strconv.Atoi(sizeStr); err == nil && s > 0 {
		pageSize = s
	}
	if pageSize > maxSize {
		pageSize = maxSize
	}
	return page, pageSize
}

func buildWhereClause(conditions []string) string {
	if len(conditions) == 0 {
		return ""
	}
	return "WHERE " + strings.Join(conditions, " AND ")
}

func writeJSON(w http.ResponseWriter, status int, payload any) {
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(status)
	if payload == nil {
		return
	}
	if err := json.NewEncoder(w).Encode(payload); err != nil {
		log.Printf("write json error: %v", err)
	}
}

func writeError(w http.ResponseWriter, status int, err error) {
	writeJSON(w, status, map[string]string{
		"error": err.Error(),
	})
}

func nullableInt(v sql.NullInt64) *int {
	if !v.Valid {
		return nil
	}
	value := int(v.Int64)
	return &value
}

func rawJSON(value []byte) json.RawMessage {
	if len(value) == 0 {
		return nil
	}
	trimmed := strings.TrimSpace(string(value))
	if trimmed == "" || strings.EqualFold(trimmed, "null") {
		return nil
	}
	return json.RawMessage([]byte(trimmed))
}

func copyStringArray(arr []string) []string {
	if len(arr) == 0 {
		return nil
	}
	out := make([]string, len(arr))
	copy(out, arr)
	return out
}

func firstNonEmpty(values ...string) string {
	for _, v := range values {
		if strings.TrimSpace(v) != "" {
			return v
		}
	}
	return ""
}
