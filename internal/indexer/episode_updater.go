package indexer

import (
	"context"
	"fmt"
	"log"
	"path/filepath"

	"animedb/internal/model"
	"animedb/internal/repository"
	"animedb/internal/video"
)

func UpdateEpisodeNumbers(ctx context.Context, repo repository.VideoRepository) (int, error) {
	episodes, err := repo.ListEpisodesWithoutEpisodeNumber(ctx)
	if err != nil {
		return 0, fmt.Errorf("list episodes without episode number: %w", err)
	}

	updated := 0
	for _, episode := range episodes {
		parsed := video.ParseFilename(episode.FilePath)
		if parsed.EpisodeNumber != nil && *parsed.EpisodeNumber >= 0 && *parsed.EpisodeNumber < 1000000 {
			episode.EpisodeNumber = parsed.EpisodeNumber
			if err := repo.UpdateEpisode(ctx, &episode); err != nil {
				log.Printf("Failed to update episode %d: %v", episode.ID, err)
				continue
			}
			updated++
			if updated%100 == 0 {
				log.Printf("Updated %d episodes...", updated)
			}
		}
	}

	return updated, nil
}

func ExtractThumbnailsForEpisodes(ctx context.Context, repo repository.VideoRepository) (int, error) {
	if !video.IsFFMpegAvailable() {
		return 0, fmt.Errorf("ffmpeg is not available")
	}

	episodes, err := repo.ListEpisodesWithoutThumbnails(ctx)
	if err != nil {
		return 0, fmt.Errorf("list episodes without thumbnails: %w", err)
	}

	extracted := 0
	for _, episode := range episodes {
		if episode.Duration == nil || *episode.Duration <= 0 {
			continue
		}

		animeFolderPath := filepath.Dir(episode.FilePath)
		thumbDir := video.GetThumbnailDirectory(animeFolderPath, episode.ID)

		thumbnails, err := repo.ListThumbnailsByEpisode(ctx, episode.ID)
		if err == nil && len(thumbnails) >= 20 {
			continue
		}

		thumbPaths, err := video.ExtractThumbnails(episode.FilePath, thumbDir, *episode.Duration)
		if err != nil {
			log.Printf("Failed to extract thumbnails for episode %d: %v", episode.ID, err)
			continue
		}

		for _, thumbPath := range thumbPaths {
			timestamp := extractTimestampFromPath(thumbPath)
			if timestamp > 0 {
				thumbnail := &model.Thumbnail{
					EpisodeID:    episode.ID,
					FilePath:     thumbPath,
					TimestampSec: timestamp,
				}
				if _, err := repo.CreateThumbnail(ctx, thumbnail); err != nil {
					log.Printf("Failed to create thumbnail for episode %d: %v", episode.ID, err)
					continue
				}
			}
		}

		extracted++
		if extracted%10 == 0 {
			log.Printf("Extracted thumbnails for %d episodes...", extracted)
		}
	}

	return extracted, nil
}

func extractTimestampFromPath(thumbPath string) float64 {
	base := filepath.Base(thumbPath)
	var timestamp float64
	if _, err := fmt.Sscanf(base, "thumb_%f.jpg", &timestamp); err == nil {
		return timestamp
	}
	return 0
}

