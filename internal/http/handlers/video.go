package handlers

import (
	"context"
	"net/http"
	"strconv"
	"time"

	"github.com/go-chi/chi/v5"

	"animedb/internal/http/response"
	"animedb/internal/model"
	"animedb/internal/repository"
	"animedb/internal/service"
)

type VideoHandlers struct {
	repo          repository.VideoRepository
	searchService *service.VideoSearchService
}

func NewVideoHandlers(repo repository.VideoRepository) *VideoHandlers {
	return &VideoHandlers{
		repo:          repo,
		searchService: service.NewVideoSearchService(repo),
	}
}

func (h *VideoHandlers) AnimeList(w http.ResponseWriter, r *http.Request) {
	ctx, cancel := context.WithTimeout(r.Context(), 15*time.Second)
	defer cancel()

	page, pageSize := response.ParsePagination(r.URL.Query().Get("page"), r.URL.Query().Get("page_size"), 20, 500)
	search := r.URL.Query().Get("search")

	animeList, total, err := h.repo.ListAnime(ctx, search, page, pageSize)
	if err != nil {
		response.WriteError(w, http.StatusInternalServerError, err)
		return
	}

	response.WriteJSON(w, http.StatusOK, response.ListResponse[model.Anime]{
		Data: animeList,
		Pagination: response.PaginationMeta{
			Page:     page,
			PageSize: pageSize,
			Total:    total,
			HasMore:  page*pageSize < total,
		},
	})
}

func (h *VideoHandlers) AnimeGet(w http.ResponseWriter, r *http.Request) {
	ctx, cancel := context.WithTimeout(r.Context(), 15*time.Second)
	defer cancel()

	idStr := chi.URLParam(r, "id")
	id, err := strconv.Atoi(idStr)
	if err != nil {
		response.WriteError(w, http.StatusBadRequest, err)
		return
	}

	anime, err := h.repo.GetAnimeByID(ctx, id)
	if err != nil {
		response.WriteError(w, http.StatusNotFound, err)
		return
	}

	response.WriteJSON(w, http.StatusOK, anime)
}

func (h *VideoHandlers) EpisodesList(w http.ResponseWriter, r *http.Request) {
	ctx, cancel := context.WithTimeout(r.Context(), 15*time.Second)
	defer cancel()

	idStr := chi.URLParam(r, "id")
	animeID, err := strconv.Atoi(idStr)
	if err != nil {
		response.WriteError(w, http.StatusBadRequest, err)
		return
	}

	episodes, err := h.repo.ListEpisodesByAnime(ctx, animeID)
	if err != nil {
		response.WriteError(w, http.StatusInternalServerError, err)
		return
	}

	response.WriteJSON(w, http.StatusOK, episodes)
}

func (h *VideoHandlers) EpisodeGet(w http.ResponseWriter, r *http.Request) {
	ctx, cancel := context.WithTimeout(r.Context(), 15*time.Second)
	defer cancel()

	idStr := chi.URLParam(r, "id")
	id, err := strconv.Atoi(idStr)
	if err != nil {
		response.WriteError(w, http.StatusBadRequest, err)
		return
	}

	episode, err := h.repo.GetEpisodeByID(ctx, id)
	if err != nil {
		response.WriteError(w, http.StatusNotFound, err)
		return
	}

	response.WriteJSON(w, http.StatusOK, episode)
}

func (h *VideoHandlers) ThumbnailsList(w http.ResponseWriter, r *http.Request) {
	ctx, cancel := context.WithTimeout(r.Context(), 15*time.Second)
	defer cancel()

	idStr := chi.URLParam(r, "id")
	episodeID, err := strconv.Atoi(idStr)
	if err != nil {
		response.WriteError(w, http.StatusBadRequest, err)
		return
	}

	thumbnails, err := h.repo.ListThumbnailsByEpisode(ctx, episodeID)
	if err != nil {
		response.WriteError(w, http.StatusInternalServerError, err)
		return
	}

	response.WriteJSON(w, http.StatusOK, thumbnails)
}

func (h *VideoHandlers) Search(w http.ResponseWriter, r *http.Request) {
	ctx, cancel := context.WithTimeout(r.Context(), 15*time.Second)
	defer cancel()

	query := r.URL.Query().Get("q")
	if query == "" {
		response.WriteJSON(w, http.StatusBadRequest, map[string]string{
			"error": "query parameter 'q' is required",
		})
		return
	}

	limitStr := r.URL.Query().Get("limit")
	limit := 10
	if l, err := strconv.Atoi(limitStr); err == nil && l > 0 && l <= 50 {
		limit = l
	}

	results, err := h.searchService.Search(ctx, query, limit)
	if err != nil {
		response.WriteError(w, http.StatusInternalServerError, err)
		return
	}

	type searchResult struct {
		ID      int     `json:"id"`
		Title   string  `json:"title"`
		Score   float64 `json:"score"`
		Matches []string `json:"matches,omitempty"`
	}

	searchResults := make([]searchResult, 0, len(results))
	for _, res := range results {
		searchResults = append(searchResults, searchResult{
			ID:      res.Anime.ID,
			Title:   res.Anime.Title,
			Score:   res.Score,
			Matches: res.Matches,
		})
	}

	response.WriteJSON(w, http.StatusOK, searchResults)
}

func (h *VideoHandlers) TriggerScan(w http.ResponseWriter, r *http.Request) {
	w.WriteHeader(http.StatusNotImplemented)
	response.WriteJSON(w, http.StatusNotImplemented, map[string]string{
		"error": "manual scan not yet implemented",
	})
}

func (h *VideoHandlers) ScanStatus(w http.ResponseWriter, r *http.Request) {
	w.WriteHeader(http.StatusNotImplemented)
	response.WriteJSON(w, http.StatusNotImplemented, map[string]string{
		"error": "scan status not yet implemented",
	})
}

