package handlers

import (
	"context"
	"fmt"
	"net/http"
	"strconv"
	"strings"
	"time"

	"github.com/go-chi/chi/v5"

	"animedb/internal/http/response"
	"animedb/internal/model"
	"animedb/internal/repository"
	"animedb/internal/service"
)

type MyAnimeListHandlers struct {
	repo repository.MyAnimeListRepository
}

func NewMyAnimeListHandlers(repo repository.MyAnimeListRepository) *MyAnimeListHandlers {
	return &MyAnimeListHandlers{repo: repo}
}

func (h *MyAnimeListHandlers) MediaList(w http.ResponseWriter, r *http.Request) {
	ctx, cancel := context.WithTimeout(r.Context(), 15*time.Second)
	defer cancel()

	page, pageSize := response.ParsePagination(r.URL.Query().Get("page"), r.URL.Query().Get("page_size"), 20, 500)

	filters := repository.MyAnimeListFilters{
		Search: r.URL.Query().Get("search"),
		Type:   r.URL.Query().Get("type"),
		Season: r.URL.Query().Get("season"),
	}

	if yearStr := strings.TrimSpace(r.URL.Query().Get("year")); yearStr != "" {
		if year, err := strconv.Atoi(yearStr); err == nil {
			filters.Year = year
		}
	}

	results, total, err := h.repo.List(ctx, filters, page, pageSize)
	if err != nil {
		response.WriteError(w, http.StatusInternalServerError, err)
		return
	}

	response.WriteJSON(w, http.StatusOK, response.ListResponse[model.MyAnimeListAnime]{
		Data: results,
		Pagination: response.PaginationMeta{
			Page:     page,
			PageSize: pageSize,
			Total:    total,
		},
	})
}

func (h *MyAnimeListHandlers) MediaGet(w http.ResponseWriter, r *http.Request) {
	ctx, cancel := context.WithTimeout(r.Context(), 15*time.Second)
	defer cancel()

	idStr := chi.URLParam(r, "id")
	id, err := strconv.Atoi(idStr)
	if err != nil || id <= 0 {
		response.WriteError(w, http.StatusBadRequest, fmt.Errorf("invalid id: %s", idStr))
		return
	}

	item, err := h.repo.GetByID(ctx, id)
	if err != nil {
		status := http.StatusInternalServerError
		if err.Error() == "sql: no rows in result set" {
			status = http.StatusNotFound
			err = fmt.Errorf("anime %d not found", id)
		}
		response.WriteError(w, status, err)
		return
	}

	response.WriteJSON(w, http.StatusOK, item)
}

func (h *MyAnimeListHandlers) MediaSearch(w http.ResponseWriter, r *http.Request) {
	ctx, cancel := context.WithTimeout(r.Context(), 15*time.Second)
	defer cancel()

	search := strings.TrimSpace(r.URL.Query().Get("search"))
	if search == "" {
		response.WriteError(w, http.StatusBadRequest, fmt.Errorf("search query required"))
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

	results, err := service.HandleImprovedMyAnimeListSearch(ctx, h.repo, search, limit)
	if err != nil {
		response.WriteError(w, http.StatusInternalServerError, err)
		return
	}

	var searchResults []searchResult
	for _, r := range results {
		result := searchResult{
			ID:      r.ID,
			Title:   firstNonEmpty(r.Title, r.TitleEnglish, r.TitleJapanese),
			English: r.TitleEnglish,
			Native:  r.TitleJapanese,
			Score:   r.Score,
		}
		searchResults = append(searchResults, result)
	}

	response.WriteJSON(w, http.StatusOK, searchResults)
}

type searchResult struct {
	ID      int     `json:"id"`
	Title   string  `json:"title,omitempty"`
	English string  `json:"english,omitempty"`
	Native  string  `json:"native,omitempty"`
	Score   float64 `json:"score"`
}
