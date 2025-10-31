package model

import (
	"time"
)

type Anime struct {
	ID        int       `json:"id"`
	Title     string    `json:"title"`
	FolderPath string   `json:"folder_path"`
	CreatedAt time.Time `json:"created_at"`
	UpdatedAt time.Time `json:"updated_at"`
}

type Episode struct {
	ID            int        `json:"id"`
	AnimeID       int        `json:"anime_id"`
	FilePath      string     `json:"file_path"`
	Filename      string     `json:"filename"`
	FileSize      int64      `json:"file_size"`
	Duration      *float64   `json:"duration,omitempty"`
	Hash          string     `json:"hash"`
	Format        string     `json:"format"`
	Resolution    string     `json:"resolution,omitempty"`
	EpisodeNumber *int       `json:"episode_number,omitempty"`
	SeasonNumber  *int       `json:"season_number,omitempty"`
	IsCorrupted   bool       `json:"is_corrupted"`
	IsPartial     bool       `json:"is_partial"`
	CreatedAt     time.Time  `json:"created_at"`
	UpdatedAt     time.Time  `json:"updated_at"`
	IndexedAt     *time.Time `json:"indexed_at,omitempty"`
}

type Thumbnail struct {
	ID          int       `json:"id"`
	EpisodeID   int       `json:"episode_id"`
	FilePath    string    `json:"file_path"`
	TimestampSec float64  `json:"timestamp_sec"`
	CreatedAt   time.Time `json:"created_at"`
}

