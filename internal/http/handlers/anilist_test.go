package handlers

import (
	"database/sql"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"testing"
	"time"

	"animedb/internal/cache"
	"animedb/internal/http/response"
	"animedb/internal/model"
	"animedb/internal/repository"

	"github.com/DATA-DOG/go-sqlmock"
	"github.com/go-chi/chi/v5"
)

func setupAniListHandler(t *testing.T) (*AniListHandlers, sqlmock.Sqlmock, *sql.DB) {
	db, mock, err := sqlmock.New(sqlmock.QueryMatcherOption(sqlmock.QueryMatcherRegexp))
	if err != nil {
		t.Fatalf("failed to create sqlmock: %v", err)
	}

	repo := repository.NewAniListRepository(db)
	c := cache.NewLRUCache(100, 5*time.Minute)
	handler := NewAniListHandlersWithCache(repo, c)
	return handler, mock, db
}

func TestAniListHandlers_MediaSearch(t *testing.T) {
	handler, mock, db := setupAniListHandler(t)
	defer db.Close()

	mock.ExpectQuery(`SELECT.*id.*title_romaji.*title_english.*title_native`).
		WithArgs("slime", 100).
		WillReturnRows(sqlmock.NewRows([]string{"id", "title_romaji", "title_english", "title_native"}).
			AddRow(1, "Slime", "Slime", ""))

	req := httptest.NewRequest("GET", "/anilist/media/search?search=slime&limit=10", nil)
	w := httptest.NewRecorder()

	router := chi.NewRouter()
	router.Get("/anilist/media/search", handler.MediaSearch)
	router.ServeHTTP(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("expected status 200, got %d", w.Code)
	}

	var searchResponse response.SearchResponse[model.SearchResult]
	if err := json.Unmarshal(w.Body.Bytes(), &searchResponse); err != nil {
		t.Fatalf("failed to unmarshal response: %v", err)
	}
	if searchResponse.Pagination.Page == 0 {
		t.Error("expected pagination metadata")
	}

	if err := mock.ExpectationsWereMet(); err != nil {
		t.Errorf("unmet expectations: %v", err)
	}
}

func TestAniListHandlers_MediaSearch_EmptyQuery(t *testing.T) {
	handler, _, db := setupAniListHandler(t)
	defer db.Close()

	req := httptest.NewRequest("GET", "/anilist/media/search?search=", nil)
	w := httptest.NewRecorder()

	router := chi.NewRouter()
	router.Get("/anilist/media/search", handler.MediaSearch)
	router.ServeHTTP(w, req)

	if w.Code != http.StatusBadRequest {
		t.Errorf("expected status 400, got %d", w.Code)
	}
}

func TestAniListHandlers_MediaSearch_CacheHit(t *testing.T) {
	handler, mock, db := setupAniListHandler(t)
	defer db.Close()

	mock.ExpectQuery(`SELECT.*id.*title_romaji.*title_english.*title_native`).
		WithArgs("slime", 100).
		WillReturnRows(sqlmock.NewRows([]string{"id", "title_romaji", "title_english", "title_native"}).
			AddRow(1, "Slime", "Slime", ""))

	req := httptest.NewRequest("GET", "/anilist/media/search?search=slime&limit=10", nil)
	w := httptest.NewRecorder()

	router := chi.NewRouter()
	router.Get("/anilist/media/search", handler.MediaSearch)
	router.ServeHTTP(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("expected status 200, got %d", w.Code)
	}

	var searchResponse response.SearchResponse[model.SearchResult]
	if err := json.Unmarshal(w.Body.Bytes(), &searchResponse); err != nil {
		t.Fatalf("failed to unmarshal response: %v", err)
	}
	if searchResponse.Pagination.Page == 0 {
		t.Error("expected pagination metadata")
	}

	if err := mock.ExpectationsWereMet(); err != nil {
		t.Errorf("unmet expectations: %v", err)
	}

	req2 := httptest.NewRequest("GET", "/anilist/media/search?search=slime&limit=10", nil)
	w2 := httptest.NewRecorder()
	router.ServeHTTP(w2, req2)

	if w2.Code != http.StatusOK {
		t.Errorf("expected status 200 on second request (should be cached), got %d", w2.Code)
	}

	var searchResponse2 response.SearchResponse[model.SearchResult]
	if err := json.Unmarshal(w2.Body.Bytes(), &searchResponse2); err != nil {
		t.Fatalf("failed to unmarshal cached response: %v", err)
	}

	if len(searchResponse2.Data) != len(searchResponse.Data) {
		t.Errorf("expected cached result to match first request, got %d vs %d results", len(searchResponse2.Data), len(searchResponse.Data))
	}
}

func TestAniListHandlers_MediaSearch_InvalidLimit(t *testing.T) {
	handler, mock, db := setupAniListHandler(t)
	defer db.Close()

	mock.ExpectQuery(`SELECT.*id.*title_romaji.*title_english.*title_native`).
		WithArgs("test", 100).
		WillReturnRows(sqlmock.NewRows([]string{"id", "title_romaji", "title_english", "title_native"}).
			AddRow(1, "Test", "Test", ""))

	req := httptest.NewRequest("GET", "/anilist/media/search?search=test&limit=invalid", nil)
	w := httptest.NewRecorder()

	router := chi.NewRouter()
	router.Get("/anilist/media/search", handler.MediaSearch)
	router.ServeHTTP(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("expected status 200, got %d", w.Code)
	}

	if err := mock.ExpectationsWereMet(); err != nil {
		t.Errorf("unmet expectations: %v", err)
	}
}

