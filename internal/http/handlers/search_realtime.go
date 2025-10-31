package handlers

import (
	"context"
	"fmt"
	"net/http"
	"sort"
	"strconv"
	"strings"
	"time"

	"animedb/internal/cache"
	"animedb/internal/http/response"
	"animedb/internal/repository"
	"animedb/internal/service"
	"animedb/internal/util"
)

type RealtimeSearchHandlers struct {
	anilistRepo repository.AniListRepository
	malRepo     repository.MyAnimeListRepository
	cache       *cache.LRUCache
}

func NewRealtimeSearchHandlers(anilistRepo repository.AniListRepository, malRepo repository.MyAnimeListRepository) *RealtimeSearchHandlers {
	return &RealtimeSearchHandlers{
		anilistRepo: anilistRepo,
		malRepo:     malRepo,
		cache:       cache.NewLRUCache(1000, 5*time.Minute),
	}
}

func NewRealtimeSearchHandlersWithCache(anilistRepo repository.AniListRepository, malRepo repository.MyAnimeListRepository, c *cache.LRUCache) *RealtimeSearchHandlers {
	return &RealtimeSearchHandlers{
		anilistRepo: anilistRepo,
		malRepo:     malRepo,
		cache:       c,
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

	cacheKey := buildCacheKey("realtime", fmt.Sprintf("%s:%s", query, source), limit)
	if cached, ok := h.cache.Get(cacheKey); ok {
		if results, ok := cached.([]RealtimeSearchResult); ok {
			response.WriteJSON(w, http.StatusOK, results)
			return
		}
	}

	var results []RealtimeSearchResult

	if source == "anilist" {
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
	} else if source == "myanimelist" {
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
	} else {
		results = h.searchBothSources(ctx, query, limit)
	}

	h.cache.Set(cacheKey, results)
	response.WriteJSON(w, http.StatusOK, results)
}

func (h *RealtimeSearchHandlers) searchBothSources(ctx context.Context, query string, limit int) []RealtimeSearchResult {
	querySeason, hasQuerySeason := util.ExtractSeasonNumber(query)
	baseQuery := util.RemoveSeasonFromQuery(query)

	searchTerm := baseQuery
	if !hasQuerySeason {
		searchTerm = query
	}

	var allCandidates []*service.Document

	anilistPrefiltered, err := h.anilistRepo.PrefilterMedia(ctx, searchTerm, 100)
	if err == nil {
		for _, result := range anilistPrefiltered {
			combinedTitle := result.TitleRomaji.String + " " + result.TitleEnglish.String + " " + result.TitleNative.String
			tokens := service.TokenizePublic(combinedTitle)
			ngramTokens := service.GenerateAllNGramsPublic(tokens, 3)
			season, _ := util.ExtractSeasonNumber(combinedTitle)

			doc := &service.Document{
				ID:           result.ID,
				Text:         combinedTitle,
				Tokens:       ngramTokens,
				TitleRomaji:  result.TitleRomaji.String,
				TitleEnglish: result.TitleEnglish.String,
				TitleNative:  result.TitleNative.String,
				SeasonNumber: season,
				Source:       "anilist",
			}
			allCandidates = append(allCandidates, doc)
		}
	}

	malPrefiltered, err := h.malRepo.PrefilterAnime(ctx, searchTerm, 100)
	if err == nil {
		for _, result := range malPrefiltered {
			combinedTitle := result.Title.String + " " + result.TitleEnglish.String + " " + result.TitleJapanese.String
			tokens := service.TokenizePublic(combinedTitle)
			ngramTokens := service.GenerateAllNGramsPublic(tokens, 3)
			season, _ := util.ExtractSeasonNumber(combinedTitle)

			doc := &service.Document{
				ID:           result.ID,
				Text:         combinedTitle,
				Tokens:       ngramTokens,
				TitleRomaji:  result.Title.String,
				TitleEnglish: result.TitleEnglish.String,
				TitleNative:  result.TitleJapanese.String,
				SeasonNumber: season,
				Source:       "myanimelist",
			}
			allCandidates = append(allCandidates, doc)
		}
	}

	if len(allCandidates) == 0 {
		return []RealtimeSearchResult{}
	}

	engine := service.NewBM25SearchEngine()
	topDocs := engine.RankTopK(query, allCandidates, querySeason, hasQuerySeason, limit)

	var results []RealtimeSearchResult
	for _, doc := range topDocs {
		results = append(results, RealtimeSearchResult{
			ID:      doc.ID,
			Title:   firstNonEmpty(doc.TitleEnglish, doc.TitleRomaji, doc.TitleNative),
			Romaji:  doc.TitleRomaji,
			English: doc.TitleEnglish,
			Native:  doc.TitleNative,
			Source:  doc.Source,
			Score:   doc.Score,
		})
	}

	sort.SliceStable(results, func(i, j int) bool {
		return results[i].Score > results[j].Score
	})

	return results
}
