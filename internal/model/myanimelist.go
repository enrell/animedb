package model

import (
	"encoding/json"
	"time"
)

type MyAnimeListAnime struct {
	ID            int             `json:"mal_id"`
	Title         string          `json:"title,omitempty"`
	TitleEnglish  string          `json:"title_english,omitempty"`
	TitleJapanese string          `json:"title_japanese,omitempty"`
	Type          string          `json:"type,omitempty"`
	Source        string          `json:"source,omitempty"`
	Episodes      *int            `json:"episodes,omitempty"`
	Status        string          `json:"status,omitempty"`
	Airing        bool            `json:"airing"`
	AiredFrom     *time.Time      `json:"aired_from,omitempty"`
	AiredTo       *time.Time      `json:"aired_to,omitempty"`
	Duration      string          `json:"duration,omitempty"`
	Rating        string          `json:"rating,omitempty"`
	Score         *float64        `json:"score,omitempty"`
	ScoredBy      *int            `json:"scored_by,omitempty"`
	Rank          *int            `json:"rank,omitempty"`
	Popularity    *int            `json:"popularity,omitempty"`
	Members       *int            `json:"members,omitempty"`
	Favorites     *int            `json:"favorites,omitempty"`
	Synopsis      string          `json:"synopsis,omitempty"`
	Background    string          `json:"background,omitempty"`
	Season        string          `json:"season,omitempty"`
	Year          *int            `json:"year,omitempty"`
	Broadcast     json.RawMessage `json:"broadcast,omitempty"`
	Titles        json.RawMessage `json:"titles,omitempty"`
	Images        json.RawMessage `json:"images,omitempty"`
	Trailer       json.RawMessage `json:"trailer,omitempty"`
	Producers     json.RawMessage `json:"producers,omitempty"`
	Licensors     json.RawMessage `json:"licensors,omitempty"`
	Studios       json.RawMessage `json:"studios,omitempty"`
	Genres        json.RawMessage `json:"genres,omitempty"`
	Themes        json.RawMessage `json:"themes,omitempty"`
	Demographics  json.RawMessage `json:"demographics,omitempty"`
	Raw           json.RawMessage `json:"raw,omitempty"`
}
