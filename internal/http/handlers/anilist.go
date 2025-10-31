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

const queryTimeout = 15 * time.Second

type AniListHandlers struct {
	repo repository.AniListRepository
}

func NewAniListHandlers(repo repository.AniListRepository) *AniListHandlers {
	return &AniListHandlers{repo: repo}
}

func (h *AniListHandlers) MediaList(w http.ResponseWriter, r *http.Request) {
	ctx, cancel := context.WithTimeout(r.Context(), queryTimeout)
	defer cancel()

	page, pageSize := response.ParsePagination(r.URL.Query().Get("page"), r.URL.Query().Get("page_size"), 20, 500)

	filters := repository.AniListFilters{
		Search:       r.URL.Query().Get("search"),
		TitleRomaji:  r.URL.Query().Get("title_romaji"),
		TitleEnglish: r.URL.Query().Get("title_english"),
		TitleNative:  r.URL.Query().Get("title_native"),
		Type:         r.URL.Query().Get("type"),
		Season:       r.URL.Query().Get("season"),
	}

	if yearStr := strings.TrimSpace(r.URL.Query().Get("season_year")); yearStr != "" {
		if year, err := strconv.Atoi(yearStr); err == nil {
			filters.SeasonYear = year
		}
	}

	results, total, err := h.repo.List(ctx, filters, page, pageSize)
	if err != nil {
		response.WriteError(w, http.StatusInternalServerError, err)
		return
	}

	response.WriteJSON(w, http.StatusOK, response.ListResponse[model.AniListMedia]{
		Data: results,
		Pagination: response.PaginationMeta{
			Page:     page,
			PageSize: pageSize,
			Total:    total,
		},
	})
}

func (h *AniListHandlers) MediaGet(w http.ResponseWriter, r *http.Request) {
	ctx, cancel := context.WithTimeout(r.Context(), queryTimeout)
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
			err = fmt.Errorf("media %d not found", id)
		}
		response.WriteError(w, status, err)
		return
	}

	response.WriteJSON(w, http.StatusOK, item)
}

func (h *AniListHandlers) MediaSearch(w http.ResponseWriter, r *http.Request) {
	ctx, cancel := context.WithTimeout(r.Context(), queryTimeout)
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

	resultsWithMeta, err := service.HandleImprovedAniListSearch(ctx, h.repo, search, limit)
	if err != nil {
		response.WriteError(w, http.StatusInternalServerError, err)
		return
	}

	var results []model.SearchResult
	for _, r := range resultsWithMeta {
		res := model.SearchResult{
			ID:      r.ID,
			Romaji:  r.TitleRomaji.String,
			English: r.TitleEnglish.String,
			Native:  r.TitleNative.String,
			Score:   r.Score,
		}
		res.Title = firstNonEmpty(r.TitleEnglish.String, r.TitleRomaji.String, r.TitleNative.String)
		results = append(results, res)
	}

	response.WriteJSON(w, http.StatusOK, results)
}

func firstNonEmpty(values ...string) string {
	for _, v := range values {
		if strings.TrimSpace(v) != "" {
			return v
		}
	}
	return ""
}
