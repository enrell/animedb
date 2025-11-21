package video

import (
	"encoding/json"
	"fmt"
	"os/exec"
	"strconv"
)

type VideoMetadata struct {
	Duration   float64
	Format     string
	Width      int
	Height     int
	Bitrate    int64
	Codec      string
	Resolution string
}

type FFProbeOutput struct {
	Format struct {
		Duration string `json:"duration"`
		Format   string `json:"format_name"`
		Bitrate  string `json:"bit_rate"`
	} `json:"format"`
	Streams []struct {
		CodecType   string `json:"codec_type"`
		CodecName   string `json:"codec_name"`
		Width       int    `json:"width"`
		Height      int    `json:"height"`
		DisplayAspectRatio string `json:"display_aspect_ratio"`
	} `json:"streams"`
}

func ExtractMetadata(filePath string) (*VideoMetadata, error) {
	cmd := exec.Command("ffprobe",
		"-v", "quiet",
		"-print_format", "json",
		"-show_format",
		"-show_streams",
		filePath,
	)

	output, err := cmd.Output()
	if err != nil {
		return nil, fmt.Errorf("ffprobe execution failed: %w", err)
	}

	var probeOutput FFProbeOutput
	if err := json.Unmarshal(output, &probeOutput); err != nil {
		return nil, fmt.Errorf("parse ffprobe output: %w", err)
	}

	metadata := &VideoMetadata{}

	if probeOutput.Format.Duration != "" {
		duration, err := strconv.ParseFloat(probeOutput.Format.Duration, 64)
		if err == nil {
			metadata.Duration = duration
		}
	}

	metadata.Format = probeOutput.Format.Format
	if probeOutput.Format.Bitrate != "" {
		bitrate, err := strconv.ParseInt(probeOutput.Format.Bitrate, 10, 64)
		if err == nil {
			metadata.Bitrate = bitrate
		}
	}

	for _, stream := range probeOutput.Streams {
		if stream.CodecType == "video" {
			metadata.Codec = stream.CodecName
			metadata.Width = stream.Width
			metadata.Height = stream.Height
			if metadata.Width > 0 && metadata.Height > 0 {
				metadata.Resolution = fmt.Sprintf("%dx%d", metadata.Width, metadata.Height)
			}
			break
		}
	}

	return metadata, nil
}

func IsFFProbeAvailable() bool {
	_, err := exec.LookPath("ffprobe")
	return err == nil
}

