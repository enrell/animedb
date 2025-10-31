package repository

import (
	"context"
	"database/sql"
	"fmt"
	"strings"

	"animedb/internal/model"
)

type MyAnimeListRepository interface {
	GetByID(ctx context.Context, id int) (model.MyAnimeListAnime, error)
	List(ctx context.Context, filters MyAnimeListFilters, page, pageSize int) ([]model.MyAnimeListAnime, int, error)
	Search(ctx context.Context, search string) ([]MyAnimeListSearchResult, error)
	Count(ctx context.Context, whereClause string, args []any) (int, error)
}

type MyAnimeListFilters struct {
	Search string
	Type   string
	Season string
	Year   int
}

type MyAnimeListSearchResult struct {
	ID            int
	Title         sql.NullString
	TitleEnglish  sql.NullString
	TitleJapanese sql.NullString
	Score         sql.NullFloat64
}

type myAnimeListRepository struct {
	db *sql.DB
}

func NewMyAnimeListRepository(db *sql.DB) MyAnimeListRepository {
	return &myAnimeListRepository{db: db}
}

const myAnimeListColumns = `
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
`

func (r *myAnimeListRepository) GetByID(ctx context.Context, id int) (model.MyAnimeListAnime, error) {
	query := fmt.Sprintf("SELECT %s FROM anime WHERE mal_id = $1", myAnimeListColumns)
	row := r.db.QueryRowContext(ctx, query, id)
	return r.scanAnime(row)
}

func (r *myAnimeListRepository) List(ctx context.Context, filters MyAnimeListFilters, page, pageSize int) ([]model.MyAnimeListAnime, int, error) {
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

func (r *myAnimeListRepository) buildFilterQuery(filters MyAnimeListFilters) (*QueryBuilder, int) {
	qb := NewQueryBuilder()
	var searchArgPos int

	if search := strings.TrimSpace(filters.Search); search != "" {
		searchArgPos = qb.AddArg(search)
		condition := fmt.Sprintf(
			"((length(normalize_title($%d)) < 3 AND normalized_name ILIKE '%%%%' || normalize_title($%d) || '%%%%') OR similarity(normalized_name, normalize_title($%d)) >= 0.30)",
			searchArgPos, searchArgPos, searchArgPos)
		qb.AddRawCondition(condition)
	}

	if animeType := strings.TrimSpace(filters.Type); animeType != "" {
		qb.AddCondition("type = $%d", animeType)
	}

	if season := strings.TrimSpace(filters.Season); season != "" {
		qb.AddCondition("season = $%d", strings.ToLower(season))
	}

	if filters.Year > 0 {
		qb.AddCondition("year = $%d", filters.Year)
	}

	return qb, searchArgPos
}

type malQueryWithArgs struct {
	SQL  string
	Args []any
}

func (r *myAnimeListRepository) buildSelectQuery(whereClause string, searchArgPos, page, pageSize int, qb *QueryBuilder) malQueryWithArgs {
	query := fmt.Sprintf("SELECT %s FROM anime", myAnimeListColumns)
	if whereClause != "" {
		query += " " + whereClause
	}

	query += r.buildOrderClause(searchArgPos)

	offset := (page - 1) * pageSize
	queryArgs, paginationClause := qb.WithPagination(pageSize, offset)
	query += paginationClause

	return malQueryWithArgs{SQL: query, Args: queryArgs}
}

func (r *myAnimeListRepository) buildOrderClause(searchArgPos int) string {
	if searchArgPos > 0 {
		return fmt.Sprintf(" ORDER BY similarity(normalized_name, normalize_title($%d)) DESC, mal_id", searchArgPos)
	}
	return " ORDER BY mal_id"
}

func (r *myAnimeListRepository) executeQuery(ctx context.Context, query string, args []any) ([]model.MyAnimeListAnime, error) {
	rows, err := r.db.QueryContext(ctx, query, args...)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var results []model.MyAnimeListAnime
	for rows.Next() {
		item, err := r.scanAnime(rows)
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

func (r *myAnimeListRepository) Search(ctx context.Context, search string) ([]MyAnimeListSearchResult, error) {
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

	rows, err := r.db.QueryContext(ctx, query, search)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var results []MyAnimeListSearchResult
	for rows.Next() {
		var result MyAnimeListSearchResult
		if err := rows.Scan(&result.ID, &result.Title, &result.TitleEnglish, &result.TitleJapanese, &result.Score); err != nil {
			return nil, err
		}
		if result.Score.Valid {
			results = append(results, result)
		}
	}

	if err := rows.Err(); err != nil {
		return nil, err
	}

	return results, nil
}

func (r *myAnimeListRepository) Count(ctx context.Context, whereClause string, args []any) (int, error) {
	query := "SELECT COUNT(*) FROM anime"
	if whereClause != "" {
		query += " " + whereClause
	}
	var total int
	if err := r.db.QueryRowContext(ctx, query, args...).Scan(&total); err != nil {
		return 0, err
	}
	return total, nil
}

func (r *myAnimeListRepository) scanAnime(s rowScanner) (model.MyAnimeListAnime, error) {
	var (
		row           model.MyAnimeListAnime
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
		return model.MyAnimeListAnime{}, err
	}

	row.Title = ScanNullString(title)
	row.TitleEnglish = ScanNullString(titleEnglish)
	row.TitleJapanese = ScanNullString(titleJapanese)
	row.Type = ScanNullString(animeType)
	row.Source = ScanNullString(source)
	row.Episodes = ScanNullInt(episodes)
	row.Status = ScanNullString(status)
	row.Duration = ScanNullString(duration)
	row.Rating = ScanNullString(rating)
	row.Score = ScanNullFloat(score)
	row.ScoredBy = ScanNullInt(scoredBy)
	row.Rank = ScanNullInt(rank)
	row.Popularity = ScanNullInt(popularity)
	row.Members = ScanNullInt(members)
	row.Favorites = ScanNullInt(favorites)
	row.Synopsis = ScanNullString(synopsis)
	row.Background = ScanNullString(background)
	row.Season = ScanNullString(season)
	row.Year = ScanNullInt(year)

	if airedFrom.Valid {
		t := airedFrom.Time.UTC()
		row.AiredFrom = &t
	}
	if airedTo.Valid {
		t := airedTo.Time.UTC()
		row.AiredTo = &t
	}

	row.Broadcast = ScanJSONField(broadcast)
	row.Titles = ScanJSONField(titles)
	row.Images = ScanJSONField(images)
	row.Trailer = ScanJSONField(trailer)
	row.Producers = ScanJSONField(producers)
	row.Licensors = ScanJSONField(licensors)
	row.Studios = ScanJSONField(studios)
	row.Genres = ScanJSONField(genres)
	row.Themes = ScanJSONField(themes)
	row.Demographics = ScanJSONField(demographics)
	row.Raw = ScanJSONField(raw)

	return row, nil
}

