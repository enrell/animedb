package repository

import (
	"context"
	"database/sql"
	"encoding/json"
	"fmt"
	"path/filepath"
	"strings"
	"time"

	"animedb/internal/model"
)

type VideoRepository interface {
	GetAnimeByID(ctx context.Context, id int) (model.Anime, error)
	GetAnimeByFolderPath(ctx context.Context, folderPath string) (*model.Anime, error)
	GetAnimeByAniListID(ctx context.Context, anilistID int) (*model.Anime, error)
	CreateAnime(ctx context.Context, title, folderPath string) (int, error)
	UpdateAnime(ctx context.Context, id int, title string) error
	UpdateAnimeCoverImage(ctx context.Context, id int, coverImageURL string) error
	UpdateAnimeAniList(ctx context.Context, id int, anilistID int, metadata *model.AniListMedia) error
	ClearAllCoverImages(ctx context.Context) error
	CleanupOrphanedAnime(ctx context.Context) error
	ListAnime(ctx context.Context, search string, page, pageSize int) ([]model.Anime, int, error)
	ListAnimeByLibrary(ctx context.Context, libraryPath string, search string, page, pageSize int) ([]model.Anime, int, error)
	GetEpisodeByID(ctx context.Context, id int) (model.Episode, error)
	GetEpisodeByPath(ctx context.Context, filePath string) (*model.Episode, error)
	CreateEpisode(ctx context.Context, episode *model.Episode) (int, error)
	UpdateEpisode(ctx context.Context, episode *model.Episode) error
	DeleteEpisode(ctx context.Context, id int) error
	ListEpisodesByAnime(ctx context.Context, animeID int, page, pageSize int) ([]model.Episode, int, error)
	ListEpisodesWithoutEpisodeNumber(ctx context.Context) ([]model.Episode, error)
	ListEpisodesWithoutThumbnails(ctx context.Context) ([]model.Episode, error)
	GetEpisodesByHash(ctx context.Context, hash string) ([]model.Episode, error)
	SearchAnime(ctx context.Context, searchTerm string, limit int) ([]model.Anime, error)
	PrefilterAnime(ctx context.Context, search string, limit int) ([]model.Anime, error)
	CreateThumbnail(ctx context.Context, thumbnail *model.Thumbnail) (int, error)
	ListThumbnailsByEpisode(ctx context.Context, episodeID int) ([]model.Thumbnail, error)
	DeleteThumbnailsByEpisode(ctx context.Context, episodeID int) error
	CreateLibrary(ctx context.Context, path string, name *string) (int, error)
	GetLibraryByID(ctx context.Context, id int) (model.Library, error)
	GetLibraryByPath(ctx context.Context, path string) (*model.Library, error)
	ListLibraries(ctx context.Context) ([]model.Library, error)
	UpdateLibrary(ctx context.Context, id int, name *string) error
	DeleteLibrary(ctx context.Context, id int) error
	GetSetting(ctx context.Context, key string) (*model.Setting, error)
	GetAllSettings(ctx context.Context) (map[string]string, error)
	SetSetting(ctx context.Context, key, value string) error
	DeleteSetting(ctx context.Context, key string) error
}

type videoRepository struct {
	db *sql.DB
}

func NewVideoRepository(db *sql.DB) VideoRepository {
	return &videoRepository{db: db}
}

func (r *videoRepository) GetAnimeByID(ctx context.Context, id int) (model.Anime, error) {
	const query = `
		SELECT id, title, folder_path, cover_image_url, anilist_id, anilist_metadata, created_at, updated_at
		FROM anime
		WHERE id = $1
	`
	var anime model.Anime
	var coverImageURL sql.NullString
	var anilistID sql.NullInt64
	var anilistMetadata sql.NullString
	err := r.db.QueryRowContext(ctx, query, id).Scan(
		&anime.ID,
		&anime.Title,
		&anime.FolderPath,
		&coverImageURL,
		&anilistID,
		&anilistMetadata,
		&anime.CreatedAt,
		&anime.UpdatedAt,
	)
	if err != nil {
		return model.Anime{}, err
	}
	if coverImageURL.Valid {
		anime.CoverImageURL = &coverImageURL.String
	}
	if anilistID.Valid {
		id := int(anilistID.Int64)
		anime.AniListID = &id
	}
	if anilistMetadata.Valid && anilistMetadata.String != "" {
		var metadata model.AniListMedia
		if err := json.Unmarshal([]byte(anilistMetadata.String), &metadata); err == nil {
			anime.AniListMetadata = &metadata
		}
	}
	return anime, nil
}

func (r *videoRepository) GetAnimeByFolderPath(ctx context.Context, folderPath string) (*model.Anime, error) {
	const query = `
		SELECT id, title, folder_path, cover_image_url, anilist_id, anilist_metadata, created_at, updated_at
		FROM anime
		WHERE folder_path = $1
	`
	var anime model.Anime
	var coverImageURL sql.NullString
	var anilistID sql.NullInt64
	var anilistMetadata sql.NullString
	err := r.db.QueryRowContext(ctx, query, folderPath).Scan(
		&anime.ID,
		&anime.Title,
		&anime.FolderPath,
		&coverImageURL,
		&anilistID,
		&anilistMetadata,
		&anime.CreatedAt,
		&anime.UpdatedAt,
	)
	if err == sql.ErrNoRows {
		return nil, nil
	}
	if err != nil {
		return nil, err
	}
	if coverImageURL.Valid {
		anime.CoverImageURL = &coverImageURL.String
	}
	if anilistID.Valid {
		id := int(anilistID.Int64)
		anime.AniListID = &id
	}
	if anilistMetadata.Valid && anilistMetadata.String != "" {
		var metadata model.AniListMedia
		if err := json.Unmarshal([]byte(anilistMetadata.String), &metadata); err == nil {
			anime.AniListMetadata = &metadata
		}
	}
	return &anime, nil
}

func (r *videoRepository) GetAnimeByAniListID(ctx context.Context, anilistID int) (*model.Anime, error) {
	const query = `
		SELECT id, title, folder_path, cover_image_url, anilist_id, anilist_metadata, created_at, updated_at
		FROM anime
		WHERE anilist_id = $1
		LIMIT 1
	`
	var anime model.Anime
	var coverImageURL sql.NullString
	var anilistIDVal sql.NullInt64
	var anilistMetadata sql.NullString
	err := r.db.QueryRowContext(ctx, query, anilistID).Scan(
		&anime.ID,
		&anime.Title,
		&anime.FolderPath,
		&coverImageURL,
		&anilistIDVal,
		&anilistMetadata,
		&anime.CreatedAt,
		&anime.UpdatedAt,
	)
	if err == sql.ErrNoRows {
		return nil, nil
	}
	if err != nil {
		return nil, err
	}
	if coverImageURL.Valid {
		anime.CoverImageURL = &coverImageURL.String
	}
	if anilistIDVal.Valid {
		id := int(anilistIDVal.Int64)
		anime.AniListID = &id
	}
	if anilistMetadata.Valid && anilistMetadata.String != "" {
		var metadata model.AniListMedia
		if err := json.Unmarshal([]byte(anilistMetadata.String), &metadata); err == nil {
			anime.AniListMetadata = &metadata
		}
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

func (r *videoRepository) UpdateAnimeCoverImage(ctx context.Context, id int, coverImageURL string) error {
	const query = `
		UPDATE anime
		SET cover_image_url = $1, updated_at = NOW()
		WHERE id = $2
	`
	_, err := r.db.ExecContext(ctx, query, coverImageURL, id)
	return err
}

func (r *videoRepository) UpdateAnimeAniList(ctx context.Context, id int, anilistID int, metadata *model.AniListMedia) error {
	var metadataJSON []byte
	var err error
	if metadata != nil {
		metadataJSON, err = json.Marshal(metadata)
		if err != nil {
			return fmt.Errorf("marshal anilist metadata: %w", err)
		}
	}
	const query = `
		UPDATE anime
		SET anilist_id = $1, anilist_metadata = $2, updated_at = NOW()
		WHERE id = $3
	`
	_, err = r.db.ExecContext(ctx, query, anilistID, metadataJSON, id)
	return err
}

func (r *videoRepository) ClearAllCoverImages(ctx context.Context) error {
	const query = `
		UPDATE anime
		SET cover_image_url = NULL, updated_at = NOW()
		WHERE cover_image_url IS NOT NULL
	`
	_, err := r.db.ExecContext(ctx, query)
	return err
}

func (r *videoRepository) CleanupOrphanedAnime(ctx context.Context) error {
	libraries, err := r.ListLibraries(ctx)
	if err != nil {
		return err
	}

	if len(libraries) == 0 {
		const deleteAllAnimeQuery = `DELETE FROM anime`
		_, err := r.db.ExecContext(ctx, deleteAllAnimeQuery)
		return err
	}

	tx, err := r.db.BeginTx(ctx, nil)
	if err != nil {
		return err
	}
	defer tx.Rollback()

	var libraryPaths []string
	var libraryPathsWithSlash []string

	for _, lib := range libraries {
		libPathAbs, err := filepath.Abs(lib.Path)
		if err != nil {
			libPathAbs = lib.Path
		}
		libraryPaths = append(libraryPaths, libPathAbs)
		libraryPaths = append(libraryPaths, lib.Path)

		libPathWithSlash := libPathAbs
		if len(libPathWithSlash) > 0 && libPathWithSlash[len(libPathWithSlash)-1] != '/' && !strings.HasSuffix(libPathWithSlash, string(filepath.Separator)) {
			libPathWithSlash += string(filepath.Separator)
		}
		libraryPathsWithSlash = append(libraryPathsWithSlash, libPathWithSlash)

		libPathWithSlash2 := lib.Path
		if len(libPathWithSlash2) > 0 && libPathWithSlash2[len(libPathWithSlash2)-1] != '/' && !strings.HasSuffix(libPathWithSlash2, string(filepath.Separator)) {
			libPathWithSlash2 += string(filepath.Separator)
		}
		libraryPathsWithSlash = append(libraryPathsWithSlash, libPathWithSlash2)
	}

	var conditions []string
	var args []interface{}
	argPos := 1

	for _, path := range libraryPaths {
		conditions = append(conditions, fmt.Sprintf("folder_path = $%d", argPos))
		args = append(args, path)
		argPos++
	}

	for _, path := range libraryPathsWithSlash {
		conditions = append(conditions, fmt.Sprintf("folder_path LIKE $%d || '%%'", argPos))
		args = append(args, path)
		argPos++
	}

	if len(conditions) == 0 {
		return tx.Commit()
	}

	deleteOrphanedQuery := fmt.Sprintf(`
		DELETE FROM anime 
		WHERE NOT (%s)
	`, strings.Join(conditions, " OR "))

	_, err = tx.ExecContext(ctx, deleteOrphanedQuery, args...)
	if err != nil {
		return err
	}

	return tx.Commit()
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
		SELECT id, title, folder_path, cover_image_url, anilist_id, anilist_metadata, created_at, updated_at
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
		var coverImageURL sql.NullString
		var anilistID sql.NullInt64
		var anilistMetadata sql.NullString
		if err := rows.Scan(&anime.ID, &anime.Title, &anime.FolderPath, &coverImageURL, &anilistID, &anilistMetadata, &anime.CreatedAt, &anime.UpdatedAt); err != nil {
			return nil, 0, err
		}
		if coverImageURL.Valid {
			anime.CoverImageURL = &coverImageURL.String
		}
		if anilistID.Valid {
			id := int(anilistID.Int64)
			anime.AniListID = &id
		}
		if anilistMetadata.Valid && anilistMetadata.String != "" {
			var metadata model.AniListMedia
			if err := json.Unmarshal([]byte(anilistMetadata.String), &metadata); err == nil {
				anime.AniListMetadata = &metadata
			}
		}
		animeList = append(animeList, anime)
	}

	if err := rows.Err(); err != nil {
		return nil, 0, err
	}

	return animeList, total, nil
}

func (r *videoRepository) ListAnimeByLibrary(ctx context.Context, libraryPath string, search string, page, pageSize int) ([]model.Anime, int, error) {
	libPathAbs, err := filepath.Abs(libraryPath)
	if err != nil {
		libPathAbs = libraryPath
	}

	libPathWithSlash := libPathAbs
	if len(libPathWithSlash) > 0 && libPathWithSlash[len(libPathWithSlash)-1] != '/' && !strings.HasSuffix(libPathWithSlash, string(filepath.Separator)) {
		libPathWithSlash += string(filepath.Separator)
	}

	var whereConditions []string
	var args []interface{}
	argPos := 1

	whereConditions = append(whereConditions, fmt.Sprintf("(folder_path LIKE $%d || '%%' OR folder_path = $%d OR folder_path LIKE $%d || '%%' OR folder_path = $%d)", argPos, argPos+1, argPos+2, argPos+3))
	args = append(args, libPathWithSlash, libPathAbs, libraryPath, libPathWithSlash)
	argPos += 4

	if search != "" {
		whereConditions = append(whereConditions, fmt.Sprintf("(similarity(normalize_title(title), normalize_title($%d)) > 0.1 OR title ILIKE '%%' || $%d || '%%')", argPos, argPos))
		args = append(args, search)
		argPos++
	}

	whereClause := ""
	if len(whereConditions) > 0 {
		whereClause = "WHERE " + strings.Join(whereConditions, " AND ")
	}

	var total int
	countQuery := fmt.Sprintf(`SELECT COUNT(*) FROM anime %s`, whereClause)
	if err := r.db.QueryRowContext(ctx, countQuery, args...).Scan(&total); err != nil {
		return nil, 0, err
	}

	var orderClause string
	if search != "" {
		orderClause = fmt.Sprintf(`ORDER BY similarity(normalize_title(title), normalize_title($%d)) DESC, id ASC`, argPos-1)
	} else {
		orderClause = `ORDER BY id ASC`
	}

	offset := (page - 1) * pageSize
	listQuery := fmt.Sprintf(`
		SELECT id, title, folder_path, cover_image_url, anilist_id, anilist_metadata, created_at, updated_at
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
		var coverImageURL sql.NullString
		var anilistID sql.NullInt64
		var anilistMetadata sql.NullString
		if err := rows.Scan(&anime.ID, &anime.Title, &anime.FolderPath, &coverImageURL, &anilistID, &anilistMetadata, &anime.CreatedAt, &anime.UpdatedAt); err != nil {
			return nil, 0, err
		}
		if coverImageURL.Valid {
			anime.CoverImageURL = &coverImageURL.String
		}
		if anilistID.Valid {
			id := int(anilistID.Int64)
			anime.AniListID = &id
		}
		if anilistMetadata.Valid && anilistMetadata.String != "" {
			var metadata model.AniListMedia
			if err := json.Unmarshal([]byte(anilistMetadata.String), &metadata); err == nil {
				anime.AniListMetadata = &metadata
			}
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

func (r *videoRepository) ListEpisodesByAnime(ctx context.Context, animeID int, page, pageSize int) ([]model.Episode, int, error) {
	countQuery := `
		SELECT COUNT(*)
		FROM episodes
		WHERE anime_id = $1
	`
	var total int
	if err := r.db.QueryRowContext(ctx, countQuery, animeID).Scan(&total); err != nil {
		return nil, 0, err
	}

	offset := (page - 1) * pageSize
	const query = `
		SELECT id, anime_id, file_path, filename, file_size, duration, hash, format, resolution,
		       episode_number, season_number, is_corrupted, is_partial, created_at, updated_at, indexed_at
		FROM episodes
		WHERE anime_id = $1
		ORDER BY 
			CASE WHEN season_number IS NOT NULL THEN season_number ELSE 0 END,
			CASE WHEN episode_number IS NOT NULL THEN episode_number ELSE 0 END,
			file_path
		LIMIT $2 OFFSET $3
	`
	rows, err := r.db.QueryContext(ctx, query, animeID, pageSize, offset)
	if err != nil {
		return nil, 0, err
	}
	defer rows.Close()

	var episodes []model.Episode
	for rows.Next() {
		episode, err := r.scanEpisodeRow(rows)
		if err != nil {
			return nil, 0, err
		}
		episodes = append(episodes, episode)
	}

	if err := rows.Err(); err != nil {
		return nil, 0, err
	}

	return episodes, total, nil
}

func (r *videoRepository) ListEpisodesWithoutEpisodeNumber(ctx context.Context) ([]model.Episode, error) {
	const query = `
		SELECT id, anime_id, file_path, filename, file_size, duration, hash, format, resolution,
		       episode_number, season_number, is_corrupted, is_partial, created_at, updated_at, indexed_at
		FROM episodes
		WHERE episode_number IS NULL
		ORDER BY id
	`
	rows, err := r.db.QueryContext(ctx, query)
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

func (r *videoRepository) ListEpisodesWithoutThumbnails(ctx context.Context) ([]model.Episode, error) {
	const query = `
		SELECT e.id, e.anime_id, e.file_path, e.filename, e.file_size, e.duration, e.hash, e.format, e.resolution,
		       e.episode_number, e.season_number, e.is_corrupted, e.is_partial, e.created_at, e.updated_at, e.indexed_at
		FROM episodes e
		LEFT JOIN (
			SELECT episode_id, COUNT(*) as thumb_count
			FROM thumbnails
			GROUP BY episode_id
		) t ON e.id = t.episode_id
		WHERE t.thumb_count IS NULL OR t.thumb_count < 20
		ORDER BY e.id
	`
	rows, err := r.db.QueryContext(ctx, query)
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

func (r *videoRepository) CreateLibrary(ctx context.Context, path string, name *string) (int, error) {
	const query = `
		INSERT INTO libraries (path, name)
		VALUES ($1, $2)
		RETURNING id
	`
	var id int
	err := r.db.QueryRowContext(ctx, query, path, name).Scan(&id)
	if err != nil {
		return 0, err
	}
	return id, nil
}

func (r *videoRepository) GetLibraryByID(ctx context.Context, id int) (model.Library, error) {
	const query = `
		SELECT id, path, name, created_at, updated_at
		FROM libraries
		WHERE id = $1
	`
	var library model.Library
	var name sql.NullString
	err := r.db.QueryRowContext(ctx, query, id).Scan(
		&library.ID,
		&library.Path,
		&name,
		&library.CreatedAt,
		&library.UpdatedAt,
	)
	if err != nil {
		return model.Library{}, err
	}
	if name.Valid {
		library.Name = &name.String
	}
	return library, nil
}

func (r *videoRepository) GetLibraryByPath(ctx context.Context, path string) (*model.Library, error) {
	const query = `
		SELECT id, path, name, created_at, updated_at
		FROM libraries
		WHERE path = $1
	`
	var library model.Library
	var name sql.NullString
	err := r.db.QueryRowContext(ctx, query, path).Scan(
		&library.ID,
		&library.Path,
		&name,
		&library.CreatedAt,
		&library.UpdatedAt,
	)
	if err == sql.ErrNoRows {
		return nil, nil
	}
	if err != nil {
		return nil, err
	}
	if name.Valid {
		library.Name = &name.String
	}
	return &library, nil
}

func (r *videoRepository) ListLibraries(ctx context.Context) ([]model.Library, error) {
	const query = `
		SELECT id, path, name, created_at, updated_at
		FROM libraries
		ORDER BY created_at ASC
	`
	rows, err := r.db.QueryContext(ctx, query)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var libraries []model.Library
	for rows.Next() {
		var library model.Library
		var name sql.NullString
		if err := rows.Scan(&library.ID, &library.Path, &name, &library.CreatedAt, &library.UpdatedAt); err != nil {
			return nil, err
		}
		if name.Valid {
			library.Name = &name.String
		}
		libraries = append(libraries, library)
	}

	if err := rows.Err(); err != nil {
		return nil, err
	}

	return libraries, nil
}

func (r *videoRepository) UpdateLibrary(ctx context.Context, id int, name *string) error {
	const query = `
		UPDATE libraries
		SET name = $1, updated_at = NOW()
		WHERE id = $2
	`
	_, err := r.db.ExecContext(ctx, query, name, id)
	return err
}

func (r *videoRepository) DeleteLibrary(ctx context.Context, id int) error {
	library, err := r.GetLibraryByID(ctx, id)
	if err != nil {
		return err
	}

	tx, err := r.db.BeginTx(ctx, nil)
	if err != nil {
		return err
	}
	defer tx.Rollback()

	libraryPathAbs, err := filepath.Abs(library.Path)
	if err != nil {
		libraryPathAbs = library.Path
	}
	
	libraryPathWithSlash := libraryPathAbs
	if len(libraryPathWithSlash) > 0 && libraryPathWithSlash[len(libraryPathWithSlash)-1] != '/' && !strings.HasSuffix(libraryPathWithSlash, string(filepath.Separator)) {
		libraryPathWithSlash += string(filepath.Separator)
	}

	deleteAnimeQuery := `
		DELETE FROM anime 
		WHERE folder_path LIKE $1 || '%'
		   OR folder_path = $2
		   OR folder_path LIKE $3 || '%'
		   OR folder_path = $4
	`
	_, err = tx.ExecContext(ctx, deleteAnimeQuery, libraryPathWithSlash, libraryPathAbs, library.Path, libraryPathWithSlash)
	if err != nil {
		return err
	}

	deleteLibraryQuery := `DELETE FROM libraries WHERE id = $1`
	_, err = tx.ExecContext(ctx, deleteLibraryQuery, id)
	if err != nil {
		return err
	}

	if err := tx.Commit(); err != nil {
		return err
	}

	remainingLibraries, err := r.ListLibraries(ctx)
	if err == nil && len(remainingLibraries) == 0 {
		cleanupCtx, cancel := context.WithTimeout(ctx, 30*time.Second)
		defer cancel()
		r.CleanupOrphanedAnime(cleanupCtx)
	}

	return nil
}

func (r *videoRepository) GetSetting(ctx context.Context, key string) (*model.Setting, error) {
	const query = `
		SELECT id, key, value, created_at, updated_at
		FROM settings
		WHERE key = $1
	`
	var setting model.Setting
	var value sql.NullString
	err := r.db.QueryRowContext(ctx, query, key).Scan(
		&setting.ID,
		&setting.Key,
		&value,
		&setting.CreatedAt,
		&setting.UpdatedAt,
	)
	if err == sql.ErrNoRows {
		return nil, nil
	}
	if err != nil {
		return nil, err
	}
	if value.Valid {
		setting.Value = &value.String
	}
	return &setting, nil
}

func (r *videoRepository) GetAllSettings(ctx context.Context) (map[string]string, error) {
	const query = `
		SELECT key, value
		FROM settings
		ORDER BY key
	`
	rows, err := r.db.QueryContext(ctx, query)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	settings := make(map[string]string)
	for rows.Next() {
		var key string
		var value sql.NullString
		if err := rows.Scan(&key, &value); err != nil {
			return nil, err
		}
		if value.Valid {
			settings[key] = value.String
		} else {
			settings[key] = ""
		}
	}

	if err := rows.Err(); err != nil {
		return nil, err
	}

	return settings, nil
}

func (r *videoRepository) SetSetting(ctx context.Context, key, value string) error {
	const query = `
		INSERT INTO settings (key, value)
		VALUES ($1, $2)
		ON CONFLICT (key) DO UPDATE SET value = $2, updated_at = NOW()
	`
	_, err := r.db.ExecContext(ctx, query, key, value)
	return err
}

func (r *videoRepository) DeleteSetting(ctx context.Context, key string) error {
	const query = `DELETE FROM settings WHERE key = $1`
	_, err := r.db.ExecContext(ctx, query, key)
	return err
}

