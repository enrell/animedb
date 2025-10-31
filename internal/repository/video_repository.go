package repository

import (
	"context"
	"database/sql"
	"fmt"
	"time"

	"animedb/internal/model"
)

type VideoRepository interface {
	GetAnimeByID(ctx context.Context, id int) (model.Anime, error)
	GetAnimeByFolderPath(ctx context.Context, folderPath string) (*model.Anime, error)
	CreateAnime(ctx context.Context, title, folderPath string) (int, error)
	UpdateAnime(ctx context.Context, id int, title string) error
	ListAnime(ctx context.Context, search string, page, pageSize int) ([]model.Anime, int, error)
	GetEpisodeByID(ctx context.Context, id int) (model.Episode, error)
	GetEpisodeByPath(ctx context.Context, filePath string) (*model.Episode, error)
	CreateEpisode(ctx context.Context, episode *model.Episode) (int, error)
	UpdateEpisode(ctx context.Context, episode *model.Episode) error
	DeleteEpisode(ctx context.Context, id int) error
	ListEpisodesByAnime(ctx context.Context, animeID int) ([]model.Episode, error)
	GetEpisodesByHash(ctx context.Context, hash string) ([]model.Episode, error)
	SearchAnime(ctx context.Context, searchTerm string, limit int) ([]model.Anime, error)
	PrefilterAnime(ctx context.Context, search string, limit int) ([]model.Anime, error)
	CreateThumbnail(ctx context.Context, thumbnail *model.Thumbnail) (int, error)
	ListThumbnailsByEpisode(ctx context.Context, episodeID int) ([]model.Thumbnail, error)
	DeleteThumbnailsByEpisode(ctx context.Context, episodeID int) error
}

type videoRepository struct {
	db *sql.DB
}

func NewVideoRepository(db *sql.DB) VideoRepository {
	return &videoRepository{db: db}
}

func (r *videoRepository) GetAnimeByID(ctx context.Context, id int) (model.Anime, error) {
	const query = `
		SELECT id, title, folder_path, created_at, updated_at
		FROM anime
		WHERE id = $1
	`
	var anime model.Anime
	err := r.db.QueryRowContext(ctx, query, id).Scan(
		&anime.ID,
		&anime.Title,
		&anime.FolderPath,
		&anime.CreatedAt,
		&anime.UpdatedAt,
	)
	if err != nil {
		return model.Anime{}, err
	}
	return anime, nil
}

func (r *videoRepository) GetAnimeByFolderPath(ctx context.Context, folderPath string) (*model.Anime, error) {
	const query = `
		SELECT id, title, folder_path, created_at, updated_at
		FROM anime
		WHERE folder_path = $1
	`
	var anime model.Anime
	err := r.db.QueryRowContext(ctx, query, folderPath).Scan(
		&anime.ID,
		&anime.Title,
		&anime.FolderPath,
		&anime.CreatedAt,
		&anime.UpdatedAt,
	)
	if err == sql.ErrNoRows {
		return nil, nil
	}
	if err != nil {
		return nil, err
	}
	return &anime, nil
}

func (r *videoRepository) CreateAnime(ctx context.Context, title, folderPath string) (int, error) {
	const query = `
		INSERT INTO anime (title, folder_path)
		VALUES ($1, $2)
		RETURNING id
	`
	var id int
	err := r.db.QueryRowContext(ctx, query, title, folderPath).Scan(&id)
	if err != nil {
		return 0, err
	}
	return id, nil
}

func (r *videoRepository) UpdateAnime(ctx context.Context, id int, title string) error {
	const query = `
		UPDATE anime
		SET title = $1, updated_at = NOW()
		WHERE id = $2
	`
	_, err := r.db.ExecContext(ctx, query, title, id)
	return err
}

func (r *videoRepository) ListAnime(ctx context.Context, search string, page, pageSize int) ([]model.Anime, int, error) {
	var whereClause string
	var args []interface{}
	argPos := 1

	if search != "" {
		whereClause = `WHERE similarity(normalize_title(title), normalize_title($1)) > 0.1 OR title ILIKE '%' || $1 || '%'`
		args = append(args, search)
		argPos++
	}

	countQuery := fmt.Sprintf(`SELECT COUNT(*) FROM anime %s`, whereClause)
	var total int
	if err := r.db.QueryRowContext(ctx, countQuery, args...).Scan(&total); err != nil {
		return nil, 0, err
	}

	var orderClause string
	if search != "" {
		orderClause = `ORDER BY similarity(normalize_title(title), normalize_title($1)) DESC, id ASC`
	} else {
		orderClause = `ORDER BY id ASC`
	}

	offset := (page - 1) * pageSize
	listQuery := fmt.Sprintf(`
		SELECT id, title, folder_path, created_at, updated_at
		FROM anime
		%s
		%s
		LIMIT $%d OFFSET $%d
	`, whereClause, orderClause, argPos, argPos+1)

	args = append(args, pageSize, offset)
	rows, err := r.db.QueryContext(ctx, listQuery, args...)
	if err != nil {
		return nil, 0, err
	}
	defer rows.Close()

	var animeList []model.Anime
	for rows.Next() {
		var anime model.Anime
		if err := rows.Scan(&anime.ID, &anime.Title, &anime.FolderPath, &anime.CreatedAt, &anime.UpdatedAt); err != nil {
			return nil, 0, err
		}
		animeList = append(animeList, anime)
	}

	if err := rows.Err(); err != nil {
		return nil, 0, err
	}

	return animeList, total, nil
}

func (r *videoRepository) GetEpisodeByID(ctx context.Context, id int) (model.Episode, error) {
	const query = `
		SELECT id, anime_id, file_path, filename, file_size, duration, hash, format, resolution,
		       episode_number, season_number, is_corrupted, is_partial, created_at, updated_at, indexed_at
		FROM episodes
		WHERE id = $1
	`
	return r.scanEpisode(ctx, query, id)
}

func (r *videoRepository) GetEpisodeByPath(ctx context.Context, filePath string) (*model.Episode, error) {
	const query = `
		SELECT id, anime_id, file_path, filename, file_size, duration, hash, format, resolution,
		       episode_number, season_number, is_corrupted, is_partial, created_at, updated_at, indexed_at
		FROM episodes
		WHERE file_path = $1
	`
	episode, err := r.scanEpisode(ctx, query, filePath)
	if err == sql.ErrNoRows {
		return nil, nil
	}
	if err != nil {
		return nil, err
	}
	return &episode, nil
}

func (r *videoRepository) scanEpisode(ctx context.Context, query string, args ...interface{}) (model.Episode, error) {
	var episode model.Episode
	var duration sql.NullFloat64
	var resolution sql.NullString
	var episodeNumber sql.NullInt64
	var seasonNumber sql.NullInt64
	var indexedAt sql.NullTime

	err := r.db.QueryRowContext(ctx, query, args...).Scan(
		&episode.ID,
		&episode.AnimeID,
		&episode.FilePath,
		&episode.Filename,
		&episode.FileSize,
		&duration,
		&episode.Hash,
		&episode.Format,
		&resolution,
		&episodeNumber,
		&seasonNumber,
		&episode.IsCorrupted,
		&episode.IsPartial,
		&episode.CreatedAt,
		&episode.UpdatedAt,
		&indexedAt,
	)
	if err != nil {
		return model.Episode{}, err
	}

	if duration.Valid {
		episode.Duration = &duration.Float64
	}
	if resolution.Valid {
		episode.Resolution = resolution.String
	}
	if episodeNumber.Valid {
		epNum := int(episodeNumber.Int64)
		episode.EpisodeNumber = &epNum
	}
	if seasonNumber.Valid {
		seasNum := int(seasonNumber.Int64)
		episode.SeasonNumber = &seasNum
	}
	if indexedAt.Valid {
		episode.IndexedAt = &indexedAt.Time
	}

	return episode, nil
}

func (r *videoRepository) CreateEpisode(ctx context.Context, episode *model.Episode) (int, error) {
	const query = `
		INSERT INTO episodes (anime_id, file_path, filename, file_size, duration, hash, format, resolution,
		                     episode_number, season_number, is_corrupted, is_partial, indexed_at)
		VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
		RETURNING id
	`
	now := time.Now()
	var id int
	err := r.db.QueryRowContext(ctx, query,
		episode.AnimeID,
		episode.FilePath,
		episode.Filename,
		episode.FileSize,
		episode.Duration,
		episode.Hash,
		episode.Format,
		episode.Resolution,
		episode.EpisodeNumber,
		episode.SeasonNumber,
		episode.IsCorrupted,
		episode.IsPartial,
		now,
	).Scan(&id)
	if err != nil {
		return 0, err
	}
	return id, nil
}

func (r *videoRepository) UpdateEpisode(ctx context.Context, episode *model.Episode) error {
	const query = `
		UPDATE episodes
		SET file_path = $1, filename = $2, file_size = $3, duration = $4, hash = $5, format = $6,
		    resolution = $7, episode_number = $8, season_number = $9, is_corrupted = $10,
		    is_partial = $11, updated_at = NOW()
		WHERE id = $12
	`
	_, err := r.db.ExecContext(ctx, query,
		episode.FilePath,
		episode.Filename,
		episode.FileSize,
		episode.Duration,
		episode.Hash,
		episode.Format,
		episode.Resolution,
		episode.EpisodeNumber,
		episode.SeasonNumber,
		episode.IsCorrupted,
		episode.IsPartial,
		episode.ID,
	)
	return err
}

func (r *videoRepository) DeleteEpisode(ctx context.Context, id int) error {
	const query = `DELETE FROM episodes WHERE id = $1`
	_, err := r.db.ExecContext(ctx, query, id)
	return err
}

func (r *videoRepository) ListEpisodesByAnime(ctx context.Context, animeID int) ([]model.Episode, error) {
	const query = `
		SELECT id, anime_id, file_path, filename, file_size, duration, hash, format, resolution,
		       episode_number, season_number, is_corrupted, is_partial, created_at, updated_at, indexed_at
		FROM episodes
		WHERE anime_id = $1
		ORDER BY season_number NULLS LAST, episode_number NULLS LAST, filename
	`
	rows, err := r.db.QueryContext(ctx, query, animeID)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var episodes []model.Episode
	for rows.Next() {
		episode, err := r.scanEpisodeRow(rows)
		if err != nil {
			return nil, err
		}
		episodes = append(episodes, episode)
	}

	if err := rows.Err(); err != nil {
		return nil, err
	}

	return episodes, nil
}

func (r *videoRepository) scanEpisodeRow(rows *sql.Rows) (model.Episode, error) {
	var episode model.Episode
	var duration sql.NullFloat64
	var resolution sql.NullString
	var episodeNumber sql.NullInt64
	var seasonNumber sql.NullInt64
	var indexedAt sql.NullTime

	err := rows.Scan(
		&episode.ID,
		&episode.AnimeID,
		&episode.FilePath,
		&episode.Filename,
		&episode.FileSize,
		&duration,
		&episode.Hash,
		&episode.Format,
		&resolution,
		&episodeNumber,
		&seasonNumber,
		&episode.IsCorrupted,
		&episode.IsPartial,
		&episode.CreatedAt,
		&episode.UpdatedAt,
		&indexedAt,
	)
	if err != nil {
		return model.Episode{}, err
	}

	if duration.Valid {
		episode.Duration = &duration.Float64
	}
	if resolution.Valid {
		episode.Resolution = resolution.String
	}
	if episodeNumber.Valid {
		epNum := int(episodeNumber.Int64)
		episode.EpisodeNumber = &epNum
	}
	if seasonNumber.Valid {
		seasNum := int(seasonNumber.Int64)
		episode.SeasonNumber = &seasNum
	}
	if indexedAt.Valid {
		episode.IndexedAt = &indexedAt.Time
	}

	return episode, nil
}

func (r *videoRepository) GetEpisodesByHash(ctx context.Context, hash string) ([]model.Episode, error) {
	const query = `
		SELECT id, anime_id, file_path, filename, file_size, duration, hash, format, resolution,
		       episode_number, season_number, is_corrupted, is_partial, created_at, updated_at, indexed_at
		FROM episodes
		WHERE hash = $1
	`
	rows, err := r.db.QueryContext(ctx, query, hash)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var episodes []model.Episode
	for rows.Next() {
		episode, err := r.scanEpisodeRow(rows)
		if err != nil {
			return nil, err
		}
		episodes = append(episodes, episode)
	}

	if err := rows.Err(); err != nil {
		return nil, err
	}

	return episodes, nil
}

func (r *videoRepository) SearchAnime(ctx context.Context, searchTerm string, limit int) ([]model.Anime, error) {
	const query = `
		SELECT id, title, folder_path, created_at, updated_at
		FROM anime
		WHERE 
			(length(normalize_title($1)) < 3
				AND title ILIKE '%' || normalize_title($1) || '%')
			OR similarity(normalize_title(title), normalize_title($1)) > 0.1
		ORDER BY similarity(normalize_title(title), normalize_title($1)) DESC, id
		LIMIT $2
	`
	rows, err := r.db.QueryContext(ctx, query, searchTerm, limit)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var results []model.Anime
	for rows.Next() {
		var anime model.Anime
		if err := rows.Scan(&anime.ID, &anime.Title, &anime.FolderPath, &anime.CreatedAt, &anime.UpdatedAt); err != nil {
			return nil, err
		}
		results = append(results, anime)
	}

	if err := rows.Err(); err != nil {
		return nil, err
	}

	return results, nil
}

func (r *videoRepository) PrefilterAnime(ctx context.Context, search string, limit int) ([]model.Anime, error) {
	return r.SearchAnime(ctx, search, limit)
}

func (r *videoRepository) CreateThumbnail(ctx context.Context, thumbnail *model.Thumbnail) (int, error) {
	const query = `
		INSERT INTO thumbnails (episode_id, file_path, timestamp_sec)
		VALUES ($1, $2, $3)
		RETURNING id
	`
	var id int
	err := r.db.QueryRowContext(ctx, query, thumbnail.EpisodeID, thumbnail.FilePath, thumbnail.TimestampSec).Scan(&id)
	if err != nil {
		return 0, err
	}
	return id, nil
}

func (r *videoRepository) ListThumbnailsByEpisode(ctx context.Context, episodeID int) ([]model.Thumbnail, error) {
	const query = `
		SELECT id, episode_id, file_path, timestamp_sec, created_at
		FROM thumbnails
		WHERE episode_id = $1
		ORDER BY timestamp_sec
	`
	rows, err := r.db.QueryContext(ctx, query, episodeID)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var thumbnails []model.Thumbnail
	for rows.Next() {
		var thumbnail model.Thumbnail
		if err := rows.Scan(&thumbnail.ID, &thumbnail.EpisodeID, &thumbnail.FilePath, &thumbnail.TimestampSec, &thumbnail.CreatedAt); err != nil {
			return nil, err
		}
		thumbnails = append(thumbnails, thumbnail)
	}

	if err := rows.Err(); err != nil {
		return nil, err
	}

	return thumbnails, nil
}

func (r *videoRepository) DeleteThumbnailsByEpisode(ctx context.Context, episodeID int) error {
	const query = `DELETE FROM thumbnails WHERE episode_id = $1`
	_, err := r.db.ExecContext(ctx, query, episodeID)
	return err
}

