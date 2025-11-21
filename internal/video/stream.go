package video

import (
	"fmt"
	"io"
	"net/http"
	"os"
	"path/filepath"
	"strconv"
	"strings"
)

func GetVideoContentType(filename string) string {
	ext := strings.ToLower(filepath.Ext(filename))
	contentTypes := map[string]string{
		".mp4":  "video/mp4",
		".mkv":  "video/x-matroska",
		".avi":  "video/x-msvideo",
		".webm": "video/webm",
		".mov":  "video/quicktime",
	}
	if ct, ok := contentTypes[ext]; ok {
		return ct
	}
	return "application/octet-stream"
}

func ServeVideoFile(w http.ResponseWriter, r *http.Request, filePath string) error {
	file, err := os.Open(filePath)
	if err != nil {
		return fmt.Errorf("open file: %w", err)
	}
	defer file.Close()

	fileInfo, err := file.Stat()
	if err != nil {
		return fmt.Errorf("stat file: %w", err)
	}

	fileSize := fileInfo.Size()
	contentType := GetVideoContentType(filePath)

	rangeHeader := r.Header.Get("Range")
	if rangeHeader == "" {
		w.Header().Set("Content-Type", contentType)
		w.Header().Set("Content-Length", strconv.FormatInt(fileSize, 10))
		w.Header().Set("Accept-Ranges", "bytes")
		w.WriteHeader(http.StatusOK)
		_, err := io.Copy(w, file)
		return err
	}

	ranges, err := parseRange(rangeHeader, fileSize)
	if err != nil || len(ranges) == 0 {
		w.Header().Set("Content-Range", fmt.Sprintf("bytes */%d", fileSize))
		w.WriteHeader(http.StatusRequestedRangeNotSatisfiable)
		return nil
	}

	if len(ranges) == 1 {
		ra := ranges[0]
		w.Header().Set("Content-Type", contentType)
		w.Header().Set("Content-Range", fmt.Sprintf("bytes %d-%d/%d", ra.start, ra.end, fileSize))
		w.Header().Set("Content-Length", strconv.FormatInt(ra.length, 10))
		w.Header().Set("Accept-Ranges", "bytes")
		w.WriteHeader(http.StatusPartialContent)

		if _, err := file.Seek(ra.start, io.SeekStart); err != nil {
			return err
		}

		limitedReader := io.LimitReader(file, ra.length)
		_, err := io.Copy(w, limitedReader)
		return err
	}

	w.Header().Set("Content-Type", "multipart/byteranges; boundary="+boundary)
	w.Header().Set("Content-Length", strconv.FormatInt(ranges.contentLength()+ranges.multipartOverhead(fileSize, contentType), 10))
	w.WriteHeader(http.StatusPartialContent)

	for _, ra := range ranges {
		part := fmt.Sprintf("\r\n--%s\r\nContent-Type: %s\r\nContent-Range: bytes %d-%d/%d\r\n\r\n",
			boundary, contentType, ra.start, ra.end, fileSize)

		if _, err := w.Write([]byte(part)); err != nil {
			return err
		}

		if _, err := file.Seek(ra.start, io.SeekStart); err != nil {
			return err
		}

		limitedReader := io.LimitReader(file, ra.length)
		if _, err := io.Copy(w, limitedReader); err != nil {
			return err
		}
	}

	_, err = w.Write([]byte("\r\n--" + boundary + "--\r\n"))
	return err
}

type byteRange struct {
	start, end, length int64
}

type byteRanges []byteRange

const boundary = "VIDEOSTREAMBOUNDARY"

func (brs byteRanges) contentLength() int64 {
	var total int64
	for _, br := range brs {
		total += br.length
	}
	return total
}

func (brs byteRanges) multipartOverhead(fileSize int64, contentType string) int64 {
	overheadPerPart := int64(len(fmt.Sprintf("\r\n--%s\r\nContent-Type: %s\r\nContent-Range: bytes 0-0/%d\r\n\r\n",
		boundary, contentType, fileSize)))
	closingOverhead := int64(len(fmt.Sprintf("\r\n--%s--\r\n", boundary)))
	return int64(len(brs))*overheadPerPart + closingOverhead
}

func parseRange(rangeHeader string, fileSize int64) (byteRanges, error) {
	if !strings.HasPrefix(rangeHeader, "bytes=") {
		return nil, fmt.Errorf("invalid range header")
	}

	rangeSpec := strings.TrimPrefix(rangeHeader, "bytes=")
	parts := strings.Split(rangeSpec, ",")
	ranges := make(byteRanges, 0, len(parts))

	for _, part := range parts {
		part = strings.TrimSpace(part)
		if part == "" {
			continue
		}

		ra := byteRange{}
		dashIndex := strings.Index(part, "-")
		if dashIndex == -1 {
			return nil, fmt.Errorf("invalid range spec: %s", part)
		}

		startStr := part[:dashIndex]
		endStr := part[dashIndex+1:]

		var err error
		if startStr == "" {
			ra.end, err = strconv.ParseInt(endStr, 10, 64)
			if err != nil {
				return nil, err
			}
			ra.start = fileSize - ra.end
			ra.end = fileSize - 1
		} else if endStr == "" {
			ra.start, err = strconv.ParseInt(startStr, 10, 64)
			if err != nil {
				return nil, err
			}
			ra.end = fileSize - 1
		} else {
			ra.start, err = strconv.ParseInt(startStr, 10, 64)
			if err != nil {
				return nil, err
			}
			ra.end, err = strconv.ParseInt(endStr, 10, 64)
			if err != nil {
				return nil, err
			}
		}

		if ra.start < 0 {
			ra.start = 0
		}
		if ra.end >= fileSize {
			ra.end = fileSize - 1
		}
		if ra.start > ra.end {
			continue
		}

		ra.length = ra.end - ra.start + 1
		ranges = append(ranges, ra)
	}

	return ranges, nil
}

