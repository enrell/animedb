package video

import (
	"os/exec"
	"strings"
)

type HardwareEncoder struct {
	Name        string
	Encoder     string
	Decoder     string
	Acceleration string
	Available   bool
}

type HardwareDetection struct {
	NVIDIA *HardwareEncoder
	Intel  *HardwareEncoder
	AMD    *HardwareEncoder
}

func DetectHardware() *HardwareDetection {
	detection := &HardwareDetection{}
	
	if isNVIDIAAvailable() {
		detection.NVIDIA = &HardwareEncoder{
			Name:        "NVIDIA NVENC",
			Encoder:     "h264_nvenc",
			Decoder:     "h264_cuvid",
			Acceleration: "cuda",
			Available:   true,
		}
	}
	
	if isIntelQSVAvailable() {
		detection.Intel = &HardwareEncoder{
			Name:        "Intel Quick Sync",
			Encoder:     "h264_qsv",
			Decoder:     "h264_qsv",
			Acceleration: "qsv",
			Available:   true,
		}
	}
	
	if isAMDAMFAvailable() {
		detection.AMD = &HardwareEncoder{
			Name:        "AMD AMF",
			Encoder:     "h264_amf",
			Decoder:     "h264_amf",
			Acceleration: "d3d11va",
			Available:   true,
		}
	}
	
	return detection
}

func (hd *HardwareDetection) GetBestEncoder() *HardwareEncoder {
	if hd.NVIDIA != nil && hd.NVIDIA.Available {
		return hd.NVIDIA
	}
	if hd.Intel != nil && hd.Intel.Available {
		return hd.Intel
	}
	if hd.AMD != nil && hd.AMD.Available {
		return hd.AMD
	}
	return nil
}

func (hd *HardwareDetection) HasHardware() bool {
	return hd.GetBestEncoder() != nil
}

func isNVIDIAAvailable() bool {
	nvidiaSmi := exec.Command("nvidia-smi")
	if err := nvidiaSmi.Run(); err != nil {
		return false
	}

	cmd := exec.Command("ffmpeg", "-hide_banner", "-hwaccels")
	output, err := cmd.Output()
	if err != nil {
		return false
	}
	hwaccels := string(output)

	cmd = exec.Command("ffmpeg", "-hide_banner", "-encoders")
	output, err = cmd.Output()
	if err != nil {
		return false
	}
	encoders := string(output)

	return strings.Contains(hwaccels, "cuda") &&
		strings.Contains(encoders, "h264_nvenc") &&
		strings.Contains(encoders, "hevc_nvenc")
}

func isIntelQSVAvailable() bool {
	cmd := exec.Command("ffmpeg", "-hide_banner", "-hwaccels")
	output, err := cmd.Output()
	if err != nil {
		return false
	}
	hwaccels := string(output)

	cmd = exec.Command("ffmpeg", "-hide_banner", "-encoders")
	output, err = cmd.Output()
	if err != nil {
		return false
	}
	encoders := string(output)

	return strings.Contains(hwaccels, "qsv") &&
		strings.Contains(encoders, "h264_qsv") &&
		strings.Contains(encoders, "hevc_qsv")
}

func isAMDAMFAvailable() bool {
	cmd := exec.Command("ffmpeg", "-hide_banner", "-hwaccels")
	output, err := cmd.Output()
	if err != nil {
		return false
	}
	hwaccels := string(output)

	cmd = exec.Command("ffmpeg", "-hide_banner", "-encoders")
	output, err = cmd.Output()
	if err != nil {
		return false
	}
	encoders := string(output)

	return (strings.Contains(hwaccels, "d3d11va") || strings.Contains(hwaccels, "dxva2")) &&
		strings.Contains(encoders, "h264_amf") &&
		strings.Contains(encoders, "hevc_amf")
}

func CheckEncoderAvailability(encoder string) bool {
	cmd := exec.Command("ffmpeg", "-hide_banner", "-encoders")
	output, err := cmd.Output()
	if err != nil {
		return false
	}
	return strings.Contains(string(output), encoder)
}

func GetDefaultHardwareEncoder() string {
	detection := DetectHardware()
	best := detection.GetBestEncoder()
	if best != nil {
		return best.Encoder
	}
	return "libx264"
}

func GetDefaultHardwareAcceleration() string {
	detection := DetectHardware()
	best := detection.GetBestEncoder()
	if best != nil {
		return best.Acceleration
	}
	return ""
}

