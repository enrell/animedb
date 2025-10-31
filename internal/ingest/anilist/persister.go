package anilist

import (
	"context"
	"database/sql"
	"encoding/json"
	"fmt"
	"time"

	"animedb/internal/util"

	"github.com/lib/pq"
)

type Persister struct {
	db *sql.DB
}

func NewPersister(db *sql.DB) *Persister {
	return &Persister{db: db}
}

func (p *Persister) PersistMedia(ctx context.Context, mediaItems []MediaDTO) (int, error) {
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

	tx, err := p.db.BeginTx(ctx, &sql.TxOptions{})
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

		params, err := p.prepareMediaParams(m)
		if err != nil {
			return rows, err
		}

		if err := p.execUpsert(ctx, stmt, params); err != nil {
			return rows, fmt.Errorf("upsert media %d: %w", m.ID, err)
		}
		rows++
	}

	if err := tx.Commit(); err != nil {
		return rows, fmt.Errorf("commit transaction: %w", err)
	}
	return rows, nil
}

type mediaParams struct {
	tagJSON       []byte
	studioJSON    []byte
	updatedAt     sql.NullTime
	episodes      sql.NullInt64
	duration      sql.NullInt64
	seasonYear    sql.NullInt64
	averageScore  sql.NullInt64
	meanScore     sql.NullInt64
	popularity    sql.NullInt64
	favourites    sql.NullInt64
	isLicensed    sql.NullBool
	media         MediaDTO
}

func (p *Persister) prepareMediaParams(m MediaDTO) (*mediaParams, error) {
	params := &mediaParams{media: m}

	tagJSON, err := json.Marshal(m.Tags)
	if err != nil {
		return nil, fmt.Errorf("marshal tags for media %d: %w", m.ID, err)
	}
	params.tagJSON = tagJSON

	studioJSON, err := json.Marshal(m.Studios)
	if err != nil {
		return nil, fmt.Errorf("marshal studios for media %d: %w", m.ID, err)
	}
	params.studioJSON = studioJSON

	if m.UpdatedAt != nil && *m.UpdatedAt > 0 {
		params.updatedAt = sql.NullTime{
			Time:  time.Unix(*m.UpdatedAt, 0).UTC(),
			Valid: true,
		}
	}

	params.episodes = toNullInt64(m.Episodes)
	params.duration = toNullInt64(m.Duration)
	params.seasonYear = toNullInt64(m.SeasonYear)
	params.averageScore = toNullInt64(m.AverageScore)
	params.meanScore = toNullInt64(m.MeanScore)
	params.popularity = toNullInt64(m.Popularity)
	params.favourites = toNullInt64(m.Favourites)

	if m.IsLicensed != nil {
		params.isLicensed = sql.NullBool{Bool: *m.IsLicensed, Valid: true}
	}

	return params, nil
}

func (p *Persister) execUpsert(ctx context.Context, stmt *sql.Stmt, params *mediaParams) error {
	m := params.media
	_, err := stmt.ExecContext(
		ctx,
		m.ID,
		m.Type,
		util.NullIfEmpty(m.Title.Romaji),
		util.NullIfEmpty(m.Title.English),
		util.NullIfEmpty(m.Title.Native),
		pq.Array(m.Synonyms),
		util.NormalizeDescription(m.Description),
		util.NullIfEmpty(m.Format),
		util.NullIfEmpty(m.Status),
		params.episodes,
		params.duration,
		util.NullIfEmpty(m.CountryOfOrigin),
		util.NullIfEmpty(m.Source),
		util.NullIfEmpty(m.Season),
		params.seasonYear,
		params.averageScore,
		params.meanScore,
		params.popularity,
		params.favourites,
		pq.Array(m.Genres),
		params.tagJSON,
		params.studioJSON,
		util.ValueOrZero(m.StartDate.Year),
		util.ValueOrZero(m.StartDate.Month),
		util.ValueOrZero(m.StartDate.Day),
		util.ValueOrZero(m.EndDate.Year),
		util.ValueOrZero(m.EndDate.Month),
		util.ValueOrZero(m.EndDate.Day),
		util.NullIfEmpty(m.CoverImage.Large),
		util.NullIfEmpty(m.BannerImage),
		params.updatedAt,
		util.NullIfEmpty(m.SiteURL),
		m.IsAdult,
		params.isLicensed,
	)
	return err
}

func toNullInt64(ptr *int) sql.NullInt64 {
	if ptr != nil {
		return sql.NullInt64{Int64: int64(*ptr), Valid: true}
	}
	return sql.NullInt64{}
}

