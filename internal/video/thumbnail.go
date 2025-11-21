package video

import (
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"strconv"
)

const thumbnailsPerEpisode = 20

func ExtractThumbnails(videoPath string, outputDir string, duration float64) ([]string, error) {
	if err := os.MkdirAll(outputDir, 0755); err != nil {
		return nil, fmt.Errorf("create output directory: %w", err)
	}

	if duration <= 0 {
		metadata, err := ExtractMetadata(videoPath)
		if err != nil {
			return nil, fmt.Errorf("get video duration: %w", err)
		}
		duration = metadata.Duration
	}

	if duration <= 0 {
		return nil, fmt.Errorf("invalid duration: %f", duration)
	}

	interval := duration / float64(thumbnailsPerEpisode+1)
	var thumbnailPaths []string

	for i := 1; i <= thumbnailsPerEpisode; i++ {
		timestamp := interval * float64(i)

		thumbFilename := fmt.Sprintf("thumb_%.3f.jpg", timestamp)
		thumbPath := filepath.Join(outputDir, thumbFilename)

		if err := extractFrame(videoPath, thumbPath, timestamp); err != nil {
			return nil, fmt.Errorf("extract frame at %.3f: %w", timestamp, err)
		}

		thumbnailPaths = append(thumbnailPaths, thumbPath)
	}

	return thumbnailPaths, nil
}

func extractFrame(videoPath, outputPath string, timestamp float64) error {
	timestampStr := strconv.FormatFloat(timestamp, 'f', 2, 64)

	cmd := exec.Command("ffmpeg",
		"-i", videoPath,
		"-ss", timestampStr,
		"-vframes", "1",
		"-q:v", "2",
		"-y",
		outputPath,
	)

	if err := cmd.Run(); err != nil {
		return fmt.Errorf("ffmpeg execution failed: %w", err)
	}

	return nil
}

func GetThumbnailDirectory(animeFolderPath string, episodeID int) string {
	return filepath.Join(animeFolderPath, ".thumbnails", fmt.Sprintf("ep_%d", episodeID))
}

func IsFFMpegAvailable() bool {
	_, err := exec.LookPath("ffmpeg")
	return err == nil
}

func ExtractCoverImage(videoPath string, outputPath string, timestamp float64) error {
	if timestamp <= 0 {
		timestamp = 60.0
	}

	timestampStr := strconv.FormatFloat(timestamp, 'f', 2, 64)

	cmd := exec.Command("ffmpeg",
		"-i", videoPath,
		"-ss", timestampStr,
		"-vframes", "1",
		"-q:v", "2",
		"-vf", "scale=400:-1",
		"-y",
		outputPath,
	)

	if err := cmd.Run(); err != nil {
		return fmt.Errorf("ffmpeg execution failed: %w", err)
	}

	return nil
}

