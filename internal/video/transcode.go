package video

import (
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"regexp"
	"strings"
)

type TranscodeOptions struct {
	InputPath          string
	OutputPath         string
	VideoCodec         string
	AudioCodec         string
	HardwareAccel      string
	HardwareEncoder    string
	Preset             string
	Resolution         string
	Tune               string
	CRF                int
	AudioBitrate       string
	Container          string
}

type TranscodeResult struct {
	OutputPath string
	Success    bool
	Error      error
}

func CanRemux(inputPath, outputContainer string) bool {
	ext := strings.ToLower(filepath.Ext(inputPath))
	videoExts := map[string]bool{
		".mp4":  true,
		".mkv":  true,
		".avi":  true,
		".webm": true,
		".mov":  true,
	}
	if !videoExts[ext] {
		return false
	}
	
	container := strings.ToLower(filepath.Ext(inputPath))
	if container == "."+outputContainer {
		return true
	}
	
	allowedRemux := map[string][]string{
		".mkv": {".mp4", ".webm"},
		".mp4": {".webm", ".mkv"},
		".webm": {".mp4", ".mkv"},
		".avi": {".mp4"},
		".mov": {".mp4"},
	}
	
	targetExt := "." + strings.TrimPrefix(strings.ToLower(outputContainer), ".")
	if targets, ok := allowedRemux[container]; ok {
		for _, t := range targets {
			if t == targetExt {
				return true
			}
		}
	}
	
	return false
}

func Remux(inputPath, outputPath string, outputContainer string) error {
	container := strings.ToLower(filepath.Ext(inputPath))
	if container == "."+outputContainer {
		return fmt.Errorf("already in target container")
	}
	
	cmd := exec.Command("ffmpeg",
		"-i", inputPath,
		"-c", "copy",
		"-avoid_negative_ts", "make_zero",
		"-y",
		outputPath,
	)
	
	cmd.Stderr = os.Stderr
	cmd.Stdout = os.Stdout
	
	return cmd.Run()
}

func Transcode(opts TranscodeOptions) error {
	args := []string{}
	
	if opts.HardwareAccel != "" && opts.HardwareEncoder != "" {
		args = append(args, "-hwaccel", opts.HardwareAccel)
		args = append(args, "-i", opts.InputPath)
	} else {
		args = append(args, "-i", opts.InputPath)
	}
	
	if opts.VideoCodec != "" {
		args = append(args, "-c:v", opts.VideoCodec)
	} else {
		args = append(args, "-c:v", "libx264")
	}
	
	if opts.Preset != "" {
		args = append(args, "-preset", opts.Preset)
	} else {
		args = append(args, "-preset", "fast")
	}
	
	if opts.CRF > 0 {
		args = append(args, "-crf", fmt.Sprintf("%d", opts.CRF))
	} else {
		args = append(args, "-crf", "23")
	}
	
	if opts.Resolution != "" {
		args = append(args, "-vf", fmt.Sprintf("scale=%s", opts.Resolution))
	}
	
	if opts.Tune != "" {
		args = append(args, "-tune", opts.Tune)
	}
	
	if opts.AudioCodec != "" {
		args = append(args, "-c:a", opts.AudioCodec)
	} else {
		args = append(args, "-c:a", "aac")
	}
	
	if opts.AudioBitrate != "" {
		args = append(args, "-b:a", opts.AudioBitrate)
	}
	
	args = append(args, "-movflags", "+faststart")
	args = append(args, "-y", opts.OutputPath)
	
	cmd := exec.Command("ffmpeg", args...)
	cmd.Stderr = os.Stderr
	cmd.Stdout = os.Stdout
	
	return cmd.Run()
}

func GetVideoInfo(inputPath string) (map[string]string, error) {
	cmd := exec.Command("ffprobe",
		"-v", "quiet",
		"-print_format", "json",
		"-show_format",
		"-show_streams",
		inputPath,
	)
	
	output, err := cmd.Output()
	if err != nil {
		return nil, fmt.Errorf("ffprobe failed: %w", err)
	}
	
	info := make(map[string]string)
	
	videoCodec := extractJSONValue(string(output), `"codec_name":\s*"([^"]+)"`, 1)
	audioCodec := extractJSONValue(string(output), `"codec_name":\s*"([^"]+)".*"codec_type":\s*"audio"`, 1)
	width := extractJSONValue(string(output), `"width":\s*(\d+)`, 1)
	height := extractJSONValue(string(output), `"height":\s*(\d+)`, 1)
	formatName := extractJSONValue(string(output), `"format_name":\s*"([^"]+)"`, 1)
	
	info["video_codec"] = videoCodec
	info["audio_codec"] = audioCodec
	info["width"] = width
	info["height"] = height
	info["format"] = formatName
	
	return info, nil
}

func extractJSONValue(jsonStr, pattern string, group int) string {
	re := regexp.MustCompile(pattern)
	matches := re.FindStringSubmatch(jsonStr)
	if len(matches) > group {
		return matches[group]
	}
	return ""
}

