package model

import "database/sql"

type SearchResult struct {
	ID      int     `json:"id"`
	Title   string  `json:"title,omitempty"`
	Romaji  string  `json:"romaji,omitempty"`
	English string  `json:"english,omitempty"`
	Native  string  `json:"native,omitempty"`
	Score   float64 `json:"score"`
}

type SearchResultWithMetadata struct {
	ID             int
	TitleRomaji    sql.NullString
	TitleEnglish   sql.NullString
	TitleNative    sql.NullString
	Score          float64
	SeasonNumber   int
	HasSeasonMatch bool
}
