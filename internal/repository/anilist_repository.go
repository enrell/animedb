package repository

import (
	"context"
	"database/sql"
	"fmt"
	"strings"

	"animedb/internal/model"
)

type AniListRepository interface {
	GetByID(ctx context.Context, id int) (model.AniListMedia, error)
	List(ctx context.Context, filters AniListFilters, page, pageSize int) ([]model.AniListMedia, int, error)
	Count(ctx context.Context, whereClause string, args []any) (int, error)
	SearchMedia(ctx context.Context, searchTerm string, limit int) ([]SearchMediaResult, error)
	PrefilterMedia(ctx context.Context, search string, limit int) ([]SearchMediaResult, error)
}

type SearchMediaResult struct {
	ID           int
	TitleRomaji  sql.NullString
	TitleEnglish sql.NullString
	TitleNative  sql.NullString
}

type AniListFilters struct {
	Search       string
	TitleRomaji  string
	TitleEnglish string
	TitleNative  string
	Type         string
	Season       string
	SeasonYear   int
}

type aniListRepository struct {
	db *sql.DB
}

func NewAniListRepository(db *sql.DB) AniListRepository {
	return &aniListRepository{db: db}
}

const aniListMediaColumns = `
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
`

func (r *aniListRepository) GetByID(ctx context.Context, id int) (model.AniListMedia, error) {
	query := fmt.Sprintf("SELECT %s FROM media WHERE id = $1", aniListMediaColumns)
	row := r.db.QueryRowContext(ctx, query, id)
	return r.scanMedia(row)
}

func (r *aniListRepository) List(ctx context.Context, filters AniListFilters, page, pageSize int) ([]model.AniListMedia, int, error) {
	qb, searchArgPos := r.buildFilterQuery(filters)
	whereClause := qb.BuildWhereClause()

	total, err := r.Count(ctx, whereClause, qb.Args())
	if err != nil {
		return nil, 0, err
	}

	query := r.buildSelectQuery(whereClause, searchArgPos, page, pageSize, qb)

	results, err := r.executeQuery(ctx, query.SQL, query.Args)
	if err != nil {
		return nil, 0, err
	}

	return results, total, nil
}

func (r *aniListRepository) buildFilterQuery(filters AniListFilters) (*QueryBuilder, int) {
	qb := NewQueryBuilder()
	var searchArgPos int

	if search := strings.TrimSpace(filters.Search); search != "" {
		searchArgPos = qb.AddArg(search)
		condition := fmt.Sprintf(
			"((length(normalize_title($%d)) < 3 AND (COALESCE(title_romaji, '')||' '||COALESCE(title_english, '')||' '||COALESCE(title_native, '')) ILIKE '%%%%' || normalize_title($%d) || '%%%%') OR similarity(normalize_title(COALESCE(title_romaji, '')||' '||COALESCE(title_english, '')||' '||COALESCE(title_native, '')), normalize_title($%d)) > 0.1)",
			searchArgPos, searchArgPos, searchArgPos)
		qb.AddRawCondition(condition)
	}

	if titleRomaji := strings.TrimSpace(filters.TitleRomaji); titleRomaji != "" {
		qb.AddCondition("title_romaji ILIKE $%d", "%"+titleRomaji+"%")
	}

	if titleEnglish := strings.TrimSpace(filters.TitleEnglish); titleEnglish != "" {
		qb.AddCondition("title_english ILIKE $%d", "%"+titleEnglish+"%")
	}

	if titleNative := strings.TrimSpace(filters.TitleNative); titleNative != "" {
		qb.AddCondition("title_native ILIKE $%d", "%"+titleNative+"%")
	}

	if mediaType := strings.TrimSpace(filters.Type); mediaType != "" {
		qb.AddCondition("type = $%d", mediaType)
	}

	if season := strings.TrimSpace(filters.Season); season != "" {
		qb.AddCondition("season = $%d", strings.ToUpper(season))
	}

	if filters.SeasonYear > 0 {
		qb.AddCondition("season_year = $%d", filters.SeasonYear)
	}

	return qb, searchArgPos
}

type queryWithArgs struct {
	SQL  string
	Args []any
}

func (r *aniListRepository) buildSelectQuery(whereClause string, searchArgPos, page, pageSize int, qb *QueryBuilder) queryWithArgs {
	query := fmt.Sprintf("SELECT %s FROM media", aniListMediaColumns)
	if whereClause != "" {
		query += " " + whereClause
	}

	query += r.buildOrderClause(searchArgPos)

	offset := (page - 1) * pageSize
	queryArgs, paginationClause := qb.WithPagination(pageSize, offset)
	query += paginationClause

	return queryWithArgs{SQL: query, Args: queryArgs}
}

func (r *aniListRepository) buildOrderClause(searchArgPos int) string {
	if searchArgPos > 0 {
		return fmt.Sprintf(" ORDER BY similarity(normalize_title(COALESCE(title_romaji, '')||' '||COALESCE(title_english, '')||' '||COALESCE(title_native, '')), normalize_title($%d)) DESC, id", searchArgPos)
	}
	return " ORDER BY id"
}

func (r *aniListRepository) executeQuery(ctx context.Context, query string, args []any) ([]model.AniListMedia, error) {
	rows, err := r.db.QueryContext(ctx, query, args...)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var results []model.AniListMedia
	for rows.Next() {
		item, err := r.scanMedia(rows)
		if err != nil {
			return nil, err
		}
		results = append(results, item)
	}

	if err := rows.Err(); err != nil {
		return nil, err
	}

	return results, nil
}

func (r *aniListRepository) Count(ctx context.Context, whereClause string, args []any) (int, error) {
	query := "SELECT COUNT(*) FROM media"
	if whereClause != "" {
		query += " " + whereClause
	}
	var total int
	if err := r.db.QueryRowContext(ctx, query, args...).Scan(&total); err != nil {
		return 0, err
	}
	return total, nil
}

func (r *aniListRepository) SearchMedia(ctx context.Context, searchTerm string, limit int) ([]SearchMediaResult, error) {
	const query = `
SELECT
	id,
	title_romaji,
	title_english,
	title_native
FROM media
WHERE 
	(length(normalize_title($1)) < 3
		AND (COALESCE(title_romaji, '')||' '||COALESCE(title_english, '')||' '||COALESCE(title_native, '')) ILIKE '%' || normalize_title($1) || '%')
	OR similarity(normalize_title(COALESCE(title_romaji, '')||' '||COALESCE(title_english, '')||' '||COALESCE(title_native, '')), normalize_title($1)) > 0.1
ORDER BY similarity(normalize_title(COALESCE(title_romaji, '')||' '||COALESCE(title_english, '')||' '||COALESCE(title_native, '')), normalize_title($1)) DESC, id
LIMIT $2;
`

	rows, err := r.db.QueryContext(ctx, query, searchTerm, limit)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var results []SearchMediaResult
	for rows.Next() {
		var result SearchMediaResult
		if err := rows.Scan(&result.ID, &result.TitleRomaji, &result.TitleEnglish, &result.TitleNative); err != nil {
			return nil, err
		}
		results = append(results, result)
	}

	if err := rows.Err(); err != nil {
		return nil, err
	}

	return results, nil
}

func (r *aniListRepository) PrefilterMedia(ctx context.Context, search string, limit int) ([]SearchMediaResult, error) {
	const query = `
SELECT
	id,
	title_romaji,
	title_english,
	title_native
FROM media
WHERE 
	(length(normalize_title($1)) < 3
		AND (COALESCE(title_romaji, '')||' '||COALESCE(title_english, '')||' '||COALESCE(title_native, '')) ILIKE '%' || normalize_title($1) || '%')
	OR similarity(normalize_title(COALESCE(title_romaji, '')||' '||COALESCE(title_english, '')||' '||COALESCE(title_native, '')), normalize_title($1)) > 0.1
ORDER BY similarity(normalize_title(COALESCE(title_romaji, '')||' '||COALESCE(title_english, '')||' '||COALESCE(title_native, '')), normalize_title($1)) DESC, id
LIMIT $2;
`

	rows, err := r.db.QueryContext(ctx, query, search, limit)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var results []SearchMediaResult
	for rows.Next() {
		var result SearchMediaResult
		if err := rows.Scan(&result.ID, &result.TitleRomaji, &result.TitleEnglish, &result.TitleNative); err != nil {
			return nil, err
		}
		results = append(results, result)
	}

	if err := rows.Err(); err != nil {
		return nil, err
	}

	return results, nil
}

type rowScanner interface {
	Scan(dest ...any) error
}

func (r *aniListRepository) scanMedia(s rowScanner) (model.AniListMedia, error) {
	var (
		media             model.AniListMedia
		titleRomaji       sql.NullString
		titleEnglish      sql.NullString
		titleNative       sql.NullString
		mediaType         sql.NullString
		description       sql.NullString
		format            sql.NullString
		status            sql.NullString
		episodes          sql.NullInt64
		duration          sql.NullInt64
		countryOfOrigin   sql.NullString
		source            sql.NullString
		season            sql.NullString
		seasonYear        sql.NullInt64
		averageScore      sql.NullInt64
		meanScore         sql.NullInt64
		popularity        sql.NullInt64
		favourites        sql.NullInt64
		coverImage        sql.NullString
		bannerImage       sql.NullString
		updatedAt         sql.NullTime
		siteURL           sql.NullString
		isLicensed        sql.NullBool
		synonyms          []byte
		genres            []byte
		tags              []byte
		studios           []byte
		startDateYear     sql.NullInt64
		startDateMonth    sql.NullInt64
		startDateDay      sql.NullInt64
		endDateYear       sql.NullInt64
		endDateMonth      sql.NullInt64
		endDateDay        sql.NullInt64
	)

	if err := s.Scan(
		&media.ID,
		&mediaType,
		&titleRomaji,
		&titleEnglish,
		&titleNative,
		&synonyms,
		&description,
		&format,
		&status,
		&episodes,
		&duration,
		&countryOfOrigin,
		&source,
		&season,
		&seasonYear,
		&averageScore,
		&meanScore,
		&popularity,
		&favourites,
		&genres,
		&tags,
		&studios,
		&startDateYear,
		&startDateMonth,
		&startDateDay,
		&endDateYear,
		&endDateMonth,
		&endDateDay,
		&coverImage,
		&bannerImage,
		&updatedAt,
		&siteURL,
		&media.IsAdult,
		&isLicensed,
	); err != nil {
		return model.AniListMedia{}, err
	}

	media.Type = ScanNullString(mediaType)
	media.Title.Romaji = ScanNullString(titleRomaji)
	media.Title.English = ScanNullString(titleEnglish)
	media.Title.Native = ScanNullString(titleNative)
	media.Description = ScanNullString(description)
	media.Format = ScanNullString(format)
	media.Status = ScanNullString(status)
	media.Episodes = ScanNullInt(episodes)
	media.Duration = ScanNullInt(duration)
	media.CountryOfOrigin = ScanNullString(countryOfOrigin)
	media.Source = ScanNullString(source)
	media.Season = ScanNullString(season)
	media.SeasonYear = ScanNullInt(seasonYear)
	media.AverageScore = ScanNullInt(averageScore)
	media.MeanScore = ScanNullInt(meanScore)
	media.Popularity = ScanNullInt(popularity)
	media.Favourites = ScanNullInt(favourites)
	media.CoverImage = ScanNullString(coverImage)
	media.BannerImage = ScanNullString(bannerImage)
	media.SiteURL = ScanNullString(siteURL)
	media.IsLicensed = ScanNullBool(isLicensed)

	if updatedAt.Valid {
		t := updatedAt.Time.UTC()
		media.UpdatedAt = &t
	}

	media.Synonyms = ScanStringArray(synonyms)
	media.Genres = ScanStringArray(genres)
	media.Tags = ScanJSONField(tags)
	media.Studios = ScanJSONField(studios)

	media.StartDate.Year, media.StartDate.Month, media.StartDate.Day = ScanPartialDate(startDateYear, startDateMonth, startDateDay)
	media.EndDate.Year, media.EndDate.Month, media.EndDate.Day = ScanPartialDate(endDateYear, endDateMonth, endDateDay)

	return media, nil
}

