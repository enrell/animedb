package model

import (
	"encoding/json"
	"time"
)

type AniListMedia struct {
	ID              int             `json:"id"`
	Type            string          `json:"type,omitempty"`
	Title           AniListTitle    `json:"title"`
	Synonyms        []string        `json:"synonyms,omitempty"`
	Description     string          `json:"description,omitempty"`
	Format          string          `json:"format,omitempty"`
	Status          string          `json:"status,omitempty"`
	Episodes        *int            `json:"episodes,omitempty"`
	Duration        *int            `json:"duration,omitempty"`
	CountryOfOrigin string          `json:"country_of_origin,omitempty"`
	Source          string          `json:"source,omitempty"`
	Season          string          `json:"season,omitempty"`
	SeasonYear      *int            `json:"season_year,omitempty"`
	AverageScore    *int            `json:"average_score,omitempty"`
	MeanScore       *int            `json:"mean_score,omitempty"`
	Popularity      *int            `json:"popularity,omitempty"`
	Favourites      *int            `json:"favourites,omitempty"`
	Genres          []string        `json:"genres,omitempty"`
	Tags            json.RawMessage `json:"tags,omitempty"`
	Studios         json.RawMessage `json:"studios,omitempty"`
	StartDate       PartialDate     `json:"start_date,omitempty"`
	EndDate         PartialDate     `json:"end_date,omitempty"`
	CoverImage      string          `json:"cover_image,omitempty"`
	BannerImage     string          `json:"banner_image,omitempty"`
	UpdatedAt       *time.Time      `json:"updated_at,omitempty"`
	SiteURL         string          `json:"site_url,omitempty"`
	IsAdult         bool            `json:"is_adult"`
	IsLicensed      *bool           `json:"is_licensed,omitempty"`
}

type AniListTitle struct {
	Romaji  string `json:"romaji,omitempty"`
	English string `json:"english,omitempty"`
	Native  string `json:"native,omitempty"`
}

type PartialDate struct {
	Year  *int `json:"year,omitempty"`
	Month *int `json:"month,omitempty"`
	Day   *int `json:"day,omitempty"`
}
