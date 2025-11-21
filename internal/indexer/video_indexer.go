package indexer

import (
	"context"
	"encoding/json"
	"fmt"
	"net/http"
	"os"
	"path/filepath"
	"regexp"
	"strings"
	"time"

	"animedb/internal/model"
	"animedb/internal/repository"
	"animedb/internal/scanner"
	"animedb/internal/service"
	"animedb/internal/video"
)

type VideoIndexer struct {
	repo            repository.VideoRepository
	aniRepo         repository.AniListRepository
	malRepo         repository.MyAnimeListRepository
	scanner         scanner.Scanner
	bm25Engine      *service.BM25SearchEngine
	workerCount     int
	skipExisting    bool
	extractThumbs   bool
	httpClient      *http.Client
	rootPath        string
}

type IndexerOptions struct {
	WorkerCount    int
	SkipExisting   bool
	ExtractThumbs  bool
	AniListRepo    repository.AniListRepository
	MyAnimeListRepo repository.MyAnimeListRepository
}

func NewVideoIndexer(repo repository.VideoRepository, opts IndexerOptions) *VideoIndexer {
	if opts.WorkerCount <= 0 {
		opts.WorkerCount = 30
	}
	return &VideoIndexer{
		repo:          repo,
		aniRepo:       opts.AniListRepo,
		malRepo:       opts.MyAnimeListRepo,
		scanner:       scanner.NewScanner(scanner.ScanOptions{WorkerCount: opts.WorkerCount}),
		bm25Engine:    service.NewBM25SearchEngine(),
		workerCount:   opts.WorkerCount,
		skipExisting:  opts.SkipExisting,
		extractThumbs: opts.ExtractThumbs,
		httpClient: &http.Client{
			Timeout: 5 * time.Second,
		},
	}
}

func (idx *VideoIndexer) IndexPath(ctx context.Context, rootPath string) error {
	rootPathAbs, err := filepath.Abs(rootPath)
	if err != nil {
		return fmt.Errorf("get absolute path: %w", err)
	}
	idx.rootPath = rootPathAbs
	return idx.scanner.Scan(ctx, rootPathAbs, idx.processFile)
}

func (idx *VideoIndexer) IndexFile(ctx context.Context, filePath string) error {
	filePathAbs, err := filepath.Abs(filePath)
	if err != nil {
		return fmt.Errorf("get absolute file path: %w", err)
	}
	
	rootPathAbs, err := filepath.Abs(filepath.Dir(filePathAbs))
	if err != nil {
		return fmt.Errorf("get absolute root path: %w", err)
	}
	idx.rootPath = rootPathAbs
	
	return idx.processFile(ctx, filePathAbs)
}

func (idx *VideoIndexer) processFile(ctx context.Context, filePath string) error {
	if idx.skipExisting {
		existing, err := idx.repo.GetEpisodeByPath(ctx, filePath)
		if err == nil && existing != nil {
			return nil
		}
	}

	filePathAbs, err := filepath.Abs(filePath)
	if err != nil {
		return fmt.Errorf("get absolute file path: %w", err)
	}

	animeFolderPath := filepath.Dir(filePathAbs)
	animeFolderPathAbs, err := filepath.Abs(animeFolderPath)
	if err != nil {
		return fmt.Errorf("get absolute folder path: %w", err)
	}

	rootPathAbs, err := filepath.Abs(idx.rootPath)
	if err != nil {
		return fmt.Errorf("get absolute root path: %w", err)
	}

	if animeFolderPathAbs == rootPathAbs {
		return nil
	}

	fileInfo, err := os.Stat(filePath)
	if err != nil {
		return fmt.Errorf("stat file: %w", err)
	}

	filename := filepath.Base(filePath)
	ext := filepath.Ext(filename)
	format := strings.TrimPrefix(ext, ".")

	folderName := filepath.Base(animeFolderPath)
	
	rawAnimeTitle := video.ExtractTitleFromPath(filePath)
	if rawAnimeTitle == "" {
		rawAnimeTitle = folderName
	}
	
	animeTitle := idx.cleanAnimeTitle(rawAnimeTitle)
	
	parsed := video.ParseFilename(filePath)
	anime, err := idx.repo.GetAnimeByFolderPath(ctx, animeFolderPathAbs)
	if err != nil {
		return fmt.Errorf("get anime by folder: %w", err)
	}

	var animeID int
	if anime == nil {
		matchResult := idx.matchAnimeWithAniList(ctx, -1, rawAnimeTitle, filePath)
		if matchResult != nil {
			animeID = matchResult.ID
		} else {
			animeID, err = idx.repo.CreateAnime(ctx, animeTitle, animeFolderPathAbs)
			if err != nil {
				return fmt.Errorf("create anime: %w", err)
			}
			idx.matchAnimeWithAniList(ctx, animeID, rawAnimeTitle, filePath)
		}
	} else {
		animeID = anime.ID
		if anime.Title != animeTitle {
			if err := idx.repo.UpdateAnime(ctx, animeID, animeTitle); err != nil {
				return fmt.Errorf("update anime: %w", err)
			}
		}
		
		needsAniListUpdate := false
		if anime.AniListID == nil {
			needsAniListUpdate = true
		} else if anime.CoverImageURL == nil || *anime.CoverImageURL == "" {
			needsAniListUpdate = true
		} else if anime.AniListMetadata == nil {
			needsAniListUpdate = true
		}
		
		if needsAniListUpdate {
			idx.matchAnimeWithAniList(ctx, animeID, rawAnimeTitle, filePath)
		}
	}

	var duration *float64
	var resolution string

	existing, err := idx.repo.GetEpisodeByPath(ctx, filePath)
	if err == nil && existing != nil && idx.skipExisting {
		if existing.Duration != nil {
			duration = existing.Duration
		}
		if existing.Resolution != "" {
			resolution = existing.Resolution
		}
		if existing.Format != "" {
			format = existing.Format
		}
	} else if video.IsFFProbeAvailable() {
		metadata, err := video.ExtractMetadata(filePath)
		if err == nil && metadata != nil {
			if metadata.Duration > 0 {
				duration = &metadata.Duration
			}
			resolution = metadata.Resolution
			if format == "" && metadata.Format != "" {
				format = metadata.Format
			}
		}
	}

	episode := &model.Episode{
		AnimeID:       animeID,
		FilePath:      filePath,
		Filename:      filename,
		FileSize:      fileInfo.Size(),
		Duration:      duration,
		Hash:          "",
		Format:        format,
		Resolution:    resolution,
		EpisodeNumber: parsed.EpisodeNumber,
		SeasonNumber:  parsed.SeasonNumber,
		IsCorrupted:   false,
		IsPartial:     false,
	}

	if existing == nil {
		episodeID, err := idx.repo.CreateEpisode(ctx, episode)
		if err != nil {
			return fmt.Errorf("create episode: %w", err)
		}
		episode.ID = episodeID
	} else {
		episode.ID = existing.ID
		if !idx.skipExisting || fileInfo.Size() != existing.FileSize {
			if err := idx.repo.UpdateEpisode(ctx, episode); err != nil {
				return fmt.Errorf("update episode: %w", err)
			}
		}
	}

	if idx.extractThumbs && video.IsFFMpegAvailable() {
		if duration != nil && *duration > 0 {
			needsThumb := idx.shouldExtractThumbnails(ctx, animeTitle)
			if needsThumb {
				go idx.extractThumbnailsAsync(ctx, episode, animeFolderPathAbs, *duration)
			}
		}
	}

	return nil
}

func (idx *VideoIndexer) extractThumbnailsAsync(ctx context.Context, episode *model.Episode, animeFolderPath string, duration float64) {
	thumbDir := video.GetThumbnailDirectory(animeFolderPath, episode.ID)

	thumbnails, err := idx.repo.ListThumbnailsByEpisode(ctx, episode.ID)
	if err == nil && len(thumbnails) >= 20 {
		return
	}

	thumbPaths, err := video.ExtractThumbnails(episode.FilePath, thumbDir, duration)
	if err != nil {
		return
	}

	for _, thumbPath := range thumbPaths {
		timestamp := idx.extractTimestampFromPath(thumbPath)
		if timestamp > 0 {
			thumbnail := &model.Thumbnail{
				EpisodeID:    episode.ID,
				FilePath:     thumbPath,
				TimestampSec: timestamp,
			}
			_, _ = idx.repo.CreateThumbnail(ctx, thumbnail)
		}
	}
}

func (idx *VideoIndexer) shouldExtractThumbnails(ctx context.Context, animeTitle string) bool {
	if idx.aniRepo != nil {
		candidates, err := idx.aniRepo.SearchMedia(ctx, animeTitle, 5)
		if err == nil && len(candidates) > 0 {
			for _, candidate := range candidates {
				media, err := idx.aniRepo.GetByID(ctx, candidate.ID)
				if err == nil && media.CoverImage != "" {
					if idx.checkImageAccessible(media.CoverImage) {
						return false
					}
				}
			}
		}
	}

	if idx.malRepo != nil {
		candidates, err := idx.malRepo.Search(ctx, animeTitle, 5)
		if err == nil && len(candidates) > 0 {
			for _, candidate := range candidates {
				anime, err := idx.malRepo.GetByID(ctx, candidate.ID)
				if err == nil {
					var images map[string]interface{}
					if err := json.Unmarshal(anime.Images, &images); err == nil {
						if jpg, ok := images["jpg"].(map[string]interface{}); ok {
							if imageURL, ok := jpg["image_url"].(string); ok && imageURL != "" {
								if idx.checkImageAccessible(imageURL) {
									return false
								}
							}
						}
					}
				}
			}
		}
	}

	return true
}

func (idx *VideoIndexer) checkImageAccessible(url string) bool {
	resp, err := idx.httpClient.Head(url)
	if err != nil {
		return false
	}
	defer resp.Body.Close()
	return resp.StatusCode == http.StatusOK
}

func (idx *VideoIndexer) extractTimestampFromPath(thumbPath string) float64 {
	base := filepath.Base(thumbPath)
	var timestamp float64
	if _, err := fmt.Sscanf(base, "thumb_%f.jpg", &timestamp); err == nil {
		return timestamp
	}
	return 0
}

func (idx *VideoIndexer) cleanAnimeTitle(title string) string {
	bracketPattern := regexp.MustCompile(`\[([^\]]+)\]`)
	title = bracketPattern.ReplaceAllString(title, "")
	
	episodeRangePattern := regexp.MustCompile(`(?i)\s*-\s*\d{2,}\s*~\s*\d{2,}\s*`)
	title = episodeRangePattern.ReplaceAllString(title, "")
	
	fileExtensionPattern := regexp.MustCompile(`(?i)\.(mkv|mp4|avi|webm|mov|flv|wmv|m4v)$`)
	title = fileExtensionPattern.ReplaceAllString(title, "")
	
	resolutionPattern := regexp.MustCompile(`(?i)\s+\d{3,4}p\s*`)
	title = resolutionPattern.ReplaceAllString(title, " ")
	
	codecPattern := regexp.MustCompile(`(?i)\s+(HEVC|H264|AVC|X264|X265|VP9|AV1|AAC|AC3|DTS|FLAC)\s*`)
	title = codecPattern.ReplaceAllString(title, " ")
	
	webPattern := regexp.MustCompile(`(?i)\s+(WEBRip|BDRip|BluRay|DVD|WEB|CR|TV|RAW)\s*`)
	title = webPattern.ReplaceAllString(title, " ")
	
	hashPattern := regexp.MustCompile(`(?i)\s+[A-F0-9]{8,}\s*`)
	title = hashPattern.ReplaceAllString(title, " ")
	
	title = strings.TrimSpace(title)
	
	multiSpacePattern := regexp.MustCompile(`\s+`)
	title = multiSpacePattern.ReplaceAllString(title, " ")
	
	title = strings.TrimSpace(title)
	return title
}

func (idx *VideoIndexer) detectFormat(filePath string) *string {
	filename := strings.ToLower(filepath.Base(filePath))
	
	if strings.Contains(filename, "movie") {
		format := "MOVIE"
		return &format
	}
	if strings.Contains(filename, "ova") {
		format := "OVA"
		return &format
	}
	if strings.Contains(filename, "special") {
		format := "SPECIAL"
		return &format
	}
	if strings.Contains(filename, "ona") {
		format := "ONA"
		return &format
	}
	
	return nil
}

func (idx *VideoIndexer) matchAnimeWithAniList(ctx context.Context, animeID int, rawAnimeTitle string, filePath string) *model.Anime {
	if idx.aniRepo == nil {
		return nil
	}

	if rawAnimeTitle == "" {
		return nil
	}

	format := idx.detectFormat(filePath)
	
	results, _, err := service.HandleImprovedAniListSearch(ctx, idx.aniRepo, rawAnimeTitle, format, 1)
	if err != nil || len(results) == 0 {
		cleanedTitle := idx.cleanAnimeTitle(rawAnimeTitle)
		if cleanedTitle == "" {
			return nil
		}
		if format != nil {
			candidates, err := idx.aniRepo.PrefilterMedia(ctx, cleanedTitle, format, 10)
			if err != nil || len(candidates) == 0 {
				return nil
			}
			if len(candidates) > 0 {
				bestMatch := candidates[0].ID
				existingAnime, err := idx.repo.GetAnimeByAniListID(ctx, bestMatch)
				if err == nil && existingAnime != nil && existingAnime.ID != animeID {
					media, err := idx.aniRepo.GetByID(ctx, bestMatch)
					if err == nil {
						idx.repo.UpdateAnimeAniList(ctx, animeID, bestMatch, &media)
						if media.CoverImage != "" {
							idx.repo.UpdateAnimeCoverImage(ctx, animeID, media.CoverImage)
						}
					}
					return existingAnime
				}
				media, err := idx.aniRepo.GetByID(ctx, bestMatch)
				if err == nil {
					idx.repo.UpdateAnimeAniList(ctx, animeID, bestMatch, &media)
					if media.CoverImage != "" {
						idx.repo.UpdateAnimeCoverImage(ctx, animeID, media.CoverImage)
					}
				}
			}
		}
		return nil
	}

	if len(results) > 0 {
		bestMatch := results[0].ID
		existingAnime, err := idx.repo.GetAnimeByAniListID(ctx, bestMatch)
		if err == nil && existingAnime != nil {
			if animeID == -1 {
				return existingAnime
			}
			if existingAnime.ID != animeID {
				media, err := idx.aniRepo.GetByID(ctx, bestMatch)
				if err == nil {
					idx.repo.UpdateAnimeAniList(ctx, animeID, bestMatch, &media)
					if media.CoverImage != "" {
						idx.repo.UpdateAnimeCoverImage(ctx, animeID, media.CoverImage)
					}
				}
				return existingAnime
			}
		}
		if animeID != -1 {
			media, err := idx.aniRepo.GetByID(ctx, bestMatch)
			if err == nil {
				idx.repo.UpdateAnimeAniList(ctx, animeID, bestMatch, &media)
				if media.CoverImage != "" {
					idx.repo.UpdateAnimeCoverImage(ctx, animeID, media.CoverImage)
				}
			}
		}
	}
	return nil
}

