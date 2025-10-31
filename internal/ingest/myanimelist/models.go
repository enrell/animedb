package myanimelist

import "encoding/json"

type JikanResponse struct {
	Pagination JikanPagination   `json:"pagination"`
	Data       []json.RawMessage `json:"data"`
	Errors     []string          `json:"error"`
}

type JikanPagination struct {
	LastVisiblePage int  `json:"last_visible_page"`
	HasNextPage     bool `json:"has_next_page"`
	CurrentPage     int  `json:"current_page"`
	Items           struct {
		Count   int `json:"count"`
		Total   int `json:"total"`
		PerPage int `json:"per_page"`
	} `json:"items"`
}

type JikanAnime struct {
	Raw             json.RawMessage `json:"-"`
	MalID           int             `json:"mal_id"`
	URL             string          `json:"url"`
	Title           string          `json:"title"`
	TitleEnglish    string          `json:"title_english"`
	TitleJapanese   string          `json:"title_japanese"`
	Type            string          `json:"type"`
	Source          string          `json:"source"`
	Episodes        *int            `json:"episodes"`
	Status          string          `json:"status"`
	Airing          bool            `json:"airing"`
	Aired           AiredInfo       `json:"aired"`
	Duration        string          `json:"duration"`
	Rating          string          `json:"rating"`
	Score           *float64        `json:"score"`
	ScoredBy        *int            `json:"scored_by"`
	Rank            *int            `json:"rank"`
	Popularity      *int            `json:"popularity"`
	Members         *int            `json:"members"`
	Favorites       *int            `json:"favorites"`
	Synopsis        string          `json:"synopsis"`
	Background      string          `json:"background"`
	Season          string          `json:"season"`
	Year            *int            `json:"year"`
	Broadcast       json.RawMessage `json:"broadcast"`
	Titles          json.RawMessage `json:"titles"`
	Images          json.RawMessage `json:"images"`
	Trailer         json.RawMessage `json:"trailer"`
	Producers       json.RawMessage `json:"producers"`
	Licensors       json.RawMessage `json:"licensors"`
	Studios         json.RawMessage `json:"studios"`
	Genres          json.RawMessage `json:"genres"`
	Themes          json.RawMessage `json:"themes"`
	Demographics    json.RawMessage `json:"demographics"`
	SeasonInt       *int            `json:"-"`
	PremieredString string          `json:"premiered"`
}

type AiredInfo struct {
	From string `json:"from"`
	To   string `json:"to"`
}

