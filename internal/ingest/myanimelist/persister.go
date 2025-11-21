package myanimelist

import (
	"context"
	"database/sql"
	"fmt"

	"animedb/internal/util"
)

type Persister struct {
	db *sql.DB
}

func NewPersister(db *sql.DB) *Persister {
	return &Persister{db: db}
}

func (p *Persister) PersistAnime(ctx context.Context, items []JikanAnime) (int, error) {
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

	tx, err := p.db.BeginTx(ctx, &sql.TxOptions{})
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

		params, err := p.prepareAnimeParams(item)
		if err != nil {
			return rows, err
		}

		if err := p.execUpsert(ctx, stmt, params); err != nil {
			return rows, fmt.Errorf("upsert anime %d: %w", item.MalID, err)
		}
		rows++
	}

	if err := tx.Commit(); err != nil {
		return rows, fmt.Errorf("commit transaction: %w", err)
	}
	return rows, nil
}

type animeParams struct {
	airedFrom sql.NullTime
	airedTo   sql.NullTime
	anime     JikanAnime
}

func (p *Persister) prepareAnimeParams(item JikanAnime) (*animeParams, error) {
	params := &animeParams{anime: item}

	airedFrom, err := util.ParseRFC3339(item.Aired.From)
	if err != nil {
		return nil, fmt.Errorf("parse aired.from for %d: %w", item.MalID, err)
	}
	params.airedFrom = airedFrom

	airedTo, err := util.ParseRFC3339(item.Aired.To)
	if err != nil {
		return nil, fmt.Errorf("parse aired.to for %d: %w", item.MalID, err)
	}
	params.airedTo = airedTo

	return params, nil
}

func (p *Persister) execUpsert(ctx context.Context, stmt *sql.Stmt, params *animeParams) error {
	item := params.anime
	_, err := stmt.ExecContext(
		ctx,
		item.MalID,
		util.NullIfEmpty(item.Title),
		util.NullIfEmpty(item.TitleEnglish),
		util.NullIfEmpty(item.TitleJapanese),
		util.NullIfEmpty(item.Type),
		util.NullIfEmpty(item.Source),
		util.NullIntPointer(item.Episodes),
		util.NullIfEmpty(item.Status),
		item.Airing,
		params.airedFrom,
		params.airedTo,
		util.NullIfEmpty(item.Duration),
		util.NullIfEmpty(item.Rating),
		util.NullFloatPointer(item.Score),
		util.NullIntPointer(item.ScoredBy),
		util.NullIntPointer(item.Rank),
		util.NullIntPointer(item.Popularity),
		util.NullIntPointer(item.Members),
		util.NullIntPointer(item.Favorites),
		util.NormalizeDescription(item.Synopsis),
		util.NormalizeDescription(item.Background),
		util.NullIfEmpty(item.Season),
		util.NullIntPointer(item.Year),
		util.EmptyJSONIfNil(item.Broadcast),
		util.EmptyJSONIfNil(item.Titles),
		util.EmptyJSONIfNil(item.Images),
		util.EmptyJSONIfNil(item.Trailer),
		util.EmptyJSONIfNil(item.Producers),
		util.EmptyJSONIfNil(item.Licensors),
		util.EmptyJSONIfNil(item.Studios),
		util.EmptyJSONIfNil(item.Genres),
		util.EmptyJSONIfNil(item.Themes),
		util.EmptyJSONIfNil(item.Demographics),
		util.EmptyJSONIfNil(item.Raw),
	)
	return err
}

