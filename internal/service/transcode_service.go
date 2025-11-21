package service

import (
	"context"
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"animedb/internal/video"
)

type TranscodeService struct {
	cacheDir string
}

type TranscodeConfig struct {
	EnableTranscoding    bool
	HardwareEncoder      string
	HardwareAcceleration string
	Preset               string
	Resolution           string
	VideoCodec           string
	AudioCodec           string
	Tune                 string
	Container            string
	RemuxOnly            bool
}

func NewTranscodeService(cacheDir string) *TranscodeService {
	if cacheDir == "" {
		cacheDir = "/tmp/animedb/transcode"
	}
	os.MkdirAll(cacheDir, 0755)
	return &TranscodeService{
		cacheDir: cacheDir,
	}
}

func (ts *TranscodeService) GetTranscodedPath(ctx context.Context, inputPath string, config TranscodeConfig) (string, error) {
	if !config.EnableTranscoding {
		return inputPath, nil
	}
	
	inputInfo, err := video.GetVideoInfo(inputPath)
	if err != nil {
		return "", fmt.Errorf("get video info: %w", err)
	}
	
	currentCodec := inputInfo["video_codec"]
	currentAudio := inputInfo["audio_codec"]
	container := strings.ToLower(strings.TrimPrefix(filepath.Ext(inputPath), "."))
	
	targetContainer := config.Container
	if targetContainer == "" {
		targetContainer = "mp4"
	}
	
	needsTranscode := false
	
	if config.RemuxOnly {
		if !video.CanRemux(inputPath, targetContainer) {
			return inputPath, nil
		}
	} else {
		browserFriendly := (currentCodec == "h264" || currentCodec == "vp8" || currentCodec == "vp9") &&
			(currentAudio == "aac" || currentAudio == "opus" || currentAudio == "vorbis") &&
			(container == "mp4" || container == "webm")
		
		if !browserFriendly || config.Resolution != "" || config.VideoCodec != "" {
			needsTranscode = true
		}
	}
	
	if !needsTranscode && video.CanRemux(inputPath, targetContainer) {
		cacheKey := ts.getCacheKey(inputPath, "remux", targetContainer)
		cachedPath := filepath.Join(ts.cacheDir, cacheKey)
		
		if _, err := os.Stat(cachedPath); err == nil {
			return cachedPath, nil
		}
		
		outputPath := cachedPath
		if err := video.Remux(inputPath, outputPath, targetContainer); err != nil {
			return "", fmt.Errorf("remux failed: %w", err)
		}
		
		return outputPath, nil
	}
	
	if !needsTranscode {
		return inputPath, nil
	}
	
	cacheKey := ts.getCacheKey(inputPath, "transcode", targetContainer)
	if config.Resolution != "" {
		cacheKey += "_" + config.Resolution
	}
	if config.VideoCodec != "" {
		cacheKey += "_" + config.VideoCodec
	}
	
	cachedPath := filepath.Join(ts.cacheDir, cacheKey)
	
	if _, err := os.Stat(cachedPath); err == nil {
		return cachedPath, nil
	}
	
	hardwareEncoder := config.HardwareEncoder
	hardwareAccel := config.HardwareAcceleration
	
	if hardwareEncoder == "auto" || hardwareEncoder == "" {
		encoder := video.GetDefaultHardwareEncoder()
		if encoder != "libx264" {
			hardwareEncoder = encoder
			hardwareAccel = video.GetDefaultHardwareAcceleration()
		} else {
			hardwareEncoder = "libx264"
			hardwareAccel = ""
		}
	}
	
	preset := config.Preset
	if preset == "" {
		preset = "fast"
	}
	
	videoCodec := config.VideoCodec
	if videoCodec == "" {
		videoCodec = hardwareEncoder
	}
	
	audioCodec := config.AudioCodec
	if audioCodec == "" {
		audioCodec = "aac"
	}
	
	resolution := config.Resolution
	if resolution != "" && !strings.Contains(resolution, ":") {
		resolution = "-1:" + resolution
	}
	
	opts := video.TranscodeOptions{
		InputPath:       inputPath,
		OutputPath:      cachedPath,
		VideoCodec:      videoCodec,
		AudioCodec:      audioCodec,
		HardwareAccel:   hardwareAccel,
		HardwareEncoder: hardwareEncoder,
		Preset:          preset,
		Resolution:      resolution,
		Tune:            config.Tune,
		Container:       targetContainer,
	}
	
	if err := video.Transcode(opts); err != nil {
		return "", fmt.Errorf("transcode failed: %w", err)
	}
	
	return cachedPath, nil
}

func (ts *TranscodeService) getCacheKey(inputPath, method, container string) string {
	hash := fmt.Sprintf("%x", []byte(inputPath))
	return fmt.Sprintf("%s_%s.%s", method, hash[:16], container)
}

func (ts *TranscodeService) ClearCache() error {
	return os.RemoveAll(ts.cacheDir)
}

