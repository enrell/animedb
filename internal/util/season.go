package util

import (
	"regexp"
	"strconv"
	"strings"
)

func ExtractSeasonNumber(query string) (int, bool) {
	patterns := []*regexp.Regexp{
		regexp.MustCompile(`(?i)(\d+)(?:st|nd|rd|th)?\s*season`),
		regexp.MustCompile(`(?i)season\s*(\d+)`),
		regexp.MustCompile(`(?i)\bs(\d+)\b`),
		regexp.MustCompile(`第(\d+)期`),
	}

	for _, re := range patterns {
		if matches := re.FindStringSubmatch(query); len(matches) > 1 {
			if num, err := strconv.Atoi(matches[1]); err == nil {
				return num, true
			}
		}
	}
	return 0, false
}

func ExtractPartNumber(query string) (int, bool) {
	patterns := []*regexp.Regexp{
		regexp.MustCompile(`(?i)(\d+)(?:st|nd|rd|th)?\s*part`),
		regexp.MustCompile(`(?i)part\s*(\d+)`),
		regexp.MustCompile(`(?i)\bp(\d+)\b`),
		regexp.MustCompile(`(?i)(\d+)(?:st|nd|rd|th)?\s*クール`),
	}

	for _, re := range patterns {
		if matches := re.FindStringSubmatch(query); len(matches) > 1 {
			if num, err := strconv.Atoi(matches[1]); err == nil {
				return num, true
			}
		}
	}
	return 0, false
}

func RemoveSeasonFromQuery(query string) string {
	patterns := []*regexp.Regexp{
		regexp.MustCompile(`(?i)\b(\d+)(?:st|nd|rd|th)?\s*season\b`),
		regexp.MustCompile(`(?i)\bseason\s*\d+\b`),
		regexp.MustCompile(`(?i)\bs\d+\b`),
		regexp.MustCompile(`第\d+期`),
	}

	result := query
	for _, re := range patterns {
		result = re.ReplaceAllString(result, "")
	}
	return strings.TrimSpace(result)
}
