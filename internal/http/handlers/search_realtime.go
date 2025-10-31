package handlers

import (
	"context"
	"fmt"
	"net/http"
	"strconv"
	"strings"
	"time"

	"animedb/internal/http/response"
	"animedb/internal/repository"
	"animedb/internal/service"
)

type RealtimeSearchHandlers struct {
	anilistRepo repository.AniListRepository
	malRepo     repository.MyAnimeListRepository
}

func NewRealtimeSearchHandlers(anilistRepo repository.AniListRepository, malRepo repository.MyAnimeListRepository) *RealtimeSearchHandlers {
	return &RealtimeSearchHandlers{
		anilistRepo: anilistRepo,
		malRepo:     malRepo,
	}
}

type RealtimeSearchResult struct {
	ID       int     `json:"id"`
	Title    string  `json:"title,omitempty"`
	Romaji   string  `json:"romaji,omitempty"`
	English  string  `json:"english,omitempty"`
	Native   string  `json:"native,omitempty"`
	Source   string  `json:"source"`
	Score    float64 `json:"score"`
}

func (h *RealtimeSearchHandlers) Search(w http.ResponseWriter, r *http.Request) {
	ctx, cancel := context.WithTimeout(r.Context(), 5*time.Second)
	defer cancel()

	query := strings.TrimSpace(r.URL.Query().Get("q"))
	if query == "" {
		response.WriteError(w, http.StatusBadRequest, fmt.Errorf("search query 'q' required"))
		return
	}

	limit := 10
	if limitStr := strings.TrimSpace(r.URL.Query().Get("limit")); limitStr != "" {
		if l, err := strconv.Atoi(limitStr); err == nil && l > 0 {
			limit = l
		}
	}
	if limit > 50 {
		limit = 50
	}

	source := strings.ToLower(strings.TrimSpace(r.URL.Query().Get("source")))
	if source == "" {
		source = "both"
	}

	if source != "both" && source != "anilist" && source != "myanimelist" {
		response.WriteError(w, http.StatusBadRequest, fmt.Errorf("source must be 'both', 'anilist', or 'myanimelist'"))
		return
	}

	var results []RealtimeSearchResult

	if source == "both" || source == "anilist" {
		anilistResults, err := service.HandleImprovedAniListSearch(ctx, h.anilistRepo, query, limit)
		if err == nil {
			for _, r := range anilistResults {
				results = append(results, RealtimeSearchResult{
					ID:      r.ID,
					Title:   firstNonEmpty(r.TitleEnglish.String, r.TitleRomaji.String, r.TitleNative.String),
					Romaji:  r.TitleRomaji.String,
					English: r.TitleEnglish.String,
					Native:  r.TitleNative.String,
					Source:  "anilist",
					Score:   r.Score,
				})
			}
		}
	}

	if source == "both" || source == "myanimelist" {
		malResults, err := service.HandleImprovedMyAnimeListSearch(ctx, h.malRepo, query, limit)
		if err == nil {
			for _, r := range malResults {
				results = append(results, RealtimeSearchResult{
					ID:      r.ID,
					Title:   firstNonEmpty(r.Title, r.TitleEnglish, r.TitleJapanese),
					Romaji:  r.Title,
					English: r.TitleEnglish,
					Native:  r.TitleJapanese,
					Source:  "myanimelist",
					Score:   r.Score,
				})
			}
		}
	}

	if len(results) > limit {
		results = results[:limit]
	}

	response.WriteJSON(w, http.StatusOK, results)
}
