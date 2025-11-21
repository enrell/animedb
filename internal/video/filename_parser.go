package video

import (
	"path/filepath"
	"regexp"
	"strings"
)

type ParsedFilename struct {
	Title         string
	EpisodeNumber *int
	SeasonNumber  *int
}

var (
	seasonEpisodePattern1 = regexp.MustCompile(`(?i)(?:s|season)[\s._-]*(\d+)[\s._-]*(?:e|ep|episode)[\s._-]*(\d+)`)
	seasonEpisodePattern2 = regexp.MustCompile(`(?i)(\d+)x(\d+)`)
	episodePattern1       = regexp.MustCompile(`(?i)(?:ep|episode)[\s._-]*(\d{1,3})(?:[-\s\[\(\.]|$)`)
	episodePattern2       = regexp.MustCompile(`(?i)[-\s](\d{1,3})(?:[-\s\[\(\.]|$)`)
	episodePattern3       = regexp.MustCompile(`(?i)[\s._-](\d{2,3})(?:[-\s\[\(\.]|$)`)
	episodePattern4       = regexp.MustCompile(`(?i)[\s._-](\d{2,3})[\s._-]`)
)

func ParseFilename(filePath string) ParsedFilename {
	filename := filepath.Base(filePath)
	ext := filepath.Ext(filename)
	baseName := strings.TrimSuffix(filename, ext)

	result := ParsedFilename{}

	bracketPattern := regexp.MustCompile(`\[([^\]]+)\]`)
	baseName = bracketPattern.ReplaceAllString(baseName, "")

	baseName = strings.TrimSpace(baseName)
	result.Title = baseName

	if matches := seasonEpisodePattern1.FindStringSubmatch(baseName); matches != nil {
		season := parseInt(matches[1])
		episode := parseInt(matches[2])
		if season > 0 {
			result.SeasonNumber = &season
		}
		if episode >= 0 && episode < 1000 {
			result.EpisodeNumber = &episode
		}
		return result
	}

	if matches := seasonEpisodePattern2.FindStringSubmatch(baseName); matches != nil {
		season := parseInt(matches[1])
		episode := parseInt(matches[2])
		if season > 0 {
			result.SeasonNumber = &season
		}
		if episode >= 0 && episode < 1000 {
			result.EpisodeNumber = &episode
		}
		return result
	}

	if matches := episodePattern1.FindStringSubmatch(baseName); matches != nil {
		episode := parseInt(matches[1])
		if episode >= 0 && episode < 1000 {
			result.EpisodeNumber = &episode
		}
		return result
	}

	if matches := episodePattern2.FindStringSubmatch(baseName); matches != nil {
		episode := parseInt(matches[1])
		if episode >= 0 && episode < 1000 {
			result.EpisodeNumber = &episode
		}
		return result
	}

	if matches := episodePattern3.FindStringSubmatch(baseName); matches != nil {
		episode := parseInt(matches[1])
		if episode >= 0 && episode < 1000 {
			result.EpisodeNumber = &episode
		}
		return result
	}

	if matches := episodePattern4.FindAllStringSubmatch(baseName, -1); len(matches) > 0 {
		lastMatch := matches[len(matches)-1]
		episode := parseInt(lastMatch[1])
		if episode >= 0 && episode < 1000 {
			result.EpisodeNumber = &episode
		}
	}

	decimalPattern := regexp.MustCompile(`(?i)(?:ep|episode)[\s._-]*(\d+)\.(\d{1,2})(?:[-\s\[\(\.]|$)|[\s._-](\d+)\.(\d{1,2})(?:[-\s\[\(\.]|$)`)
	if matches := decimalPattern.FindStringSubmatch(baseName); matches != nil {
		var wholePart, decimalPart int
		if matches[1] != "" {
			wholePart = parseInt(matches[1])
			decimalPart = parseInt(matches[2])
		} else {
			wholePart = parseInt(matches[3])
			decimalPart = parseInt(matches[4])
		}
		if wholePart >= 0 && wholePart < 1000 && decimalPart >= 0 && decimalPart < 100 {
			episodeValue := wholePart*1000 + decimalPart*10
			if episodeValue >= 0 && episodeValue < 1000000 {
				result.EpisodeNumber = &episodeValue
			}
		}
	}

	titleParts := strings.Fields(baseName)
	if len(titleParts) > 0 {
		result.Title = strings.Join(titleParts, " ")
	}

	return result
}

func parseInt(s string) int {
	var result int
	for _, r := range s {
		if r >= '0' && r <= '9' {
			result = result*10 + int(r-'0')
		}
	}
	return result
}

func ExtractTitleFromPath(filePath string) string {
	dir := filepath.Dir(filePath)
	baseDir := filepath.Base(dir)

	rootDir := filepath.VolumeName(filePath)
	if rootDir != "" {
		parts := strings.Split(dir, string(filepath.Separator))
		if len(parts) > 1 {
			baseDir = parts[len(parts)-1]
		}
	}

	if baseDir == "" || baseDir == "." {
		return ""
	}

	bracketPattern := regexp.MustCompile(`\[([^\]]+)\]`)
	baseDir = bracketPattern.ReplaceAllString(baseDir, "")
	baseDir = strings.TrimSpace(baseDir)

	return baseDir
}

