package main

import (
	"database/sql"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"testing"
	"time"

	"github.com/DATA-DOG/go-sqlmock"
	"github.com/lib/pq"
)

func newMockDB(t *testing.T) (*sql.DB, sqlmock.Sqlmock) {
	t.Helper()
	db, mock, err := sqlmock.New(sqlmock.QueryMatcherOption(sqlmock.QueryMatcherRegexp))
	if err != nil {
		t.Fatalf("sqlmock.New: %v", err)
	}
	return db, mock
}

func TestAniListMediaList(t *testing.T) {
	aniDB, aniMock := newMockDB(t)
	defer aniDB.Close()
	malDB, malMock := newMockDB(t)
	defer malDB.Close()
	defer func() {
		if err := malMock.ExpectationsWereMet(); err != nil {
			t.Fatalf("unexpected MyAnimeList expectations: %v", err)
		}
	}()

	srv := &server{anilistDB: aniDB, myAnimeListDB: malDB}

	aniMock.ExpectQuery(`SELECT\s+COUNT\(\*\)\s+FROM\s+media`).
		WillReturnRows(sqlmock.NewRows([]string{"count"}).AddRow(1))

	now := time.Now().UTC()
	columns := []string{
		"id", "type", "title_romaji", "title_english", "title_native",
		"synonyms", "description", "format", "status",
		"episodes", "duration", "country_of_origin", "source", "season",
		"season_year", "average_score", "mean_score", "popularity", "favourites",
		"genres", "tags", "studios",
		"start_date_year", "start_date_month", "start_date_day",
		"end_date_year", "end_date_month", "end_date_day",
		"cover_image", "banner_image", "updated_at", "site_url",
		"is_adult", "is_licensed",
	}
	rows := sqlmock.NewRows(columns).AddRow(
		1, "ANIME", "Romaji", "English", "Native",
		nil, "Description", "TV", "FINISHED",
		24, 24, "JP", "MANGA", "SPRING",
		2020, 90, 85, 100000, 5000,
		nil, []byte(`[{"id":1}]`), []byte(`[{"id":2}]`),
		2020, 4, 1,
		2020, 6, 30,
		"cover.jpg", "banner.jpg", now, "https://example.com",
		false, true,
	)

	aniMock.ExpectQuery(`(?s)SELECT\s+id.*FROM\s+media.*ORDER BY id LIMIT \$1 OFFSET \$2`).
		WithArgs(20, 0).
		WillReturnRows(rows)

	req := httptest.NewRequest(http.MethodGet, "/anilist/media", nil)
	rec := httptest.NewRecorder()
	srv.routes().ServeHTTP(rec, req)

	if rec.Code != http.StatusOK {
		t.Fatalf("expected status 200, got %d: %s", rec.Code, rec.Body.String())
	}

	var payload listResponse[aniListMedia]
	if err := json.NewDecoder(rec.Body).Decode(&payload); err != nil {
		t.Fatalf("decode: %v", err)
	}

	if payload.Pagination.Total != 1 {
		t.Fatalf("expected total 1, got %d", payload.Pagination.Total)
	}
	if len(payload.Data) != 1 {
		t.Fatalf("expected 1 record, got %d", len(payload.Data))
	}
	if payload.Data[0].ID != 1 || payload.Data[0].Title.English != "English" {
		t.Fatalf("unexpected payload: %#v", payload.Data[0])
	}

	if err := aniMock.ExpectationsWereMet(); err != nil {
		t.Fatalf("unexpected AniList expectations: %v", err)
	}
}

func TestAniListMediaListTitleFilters(t *testing.T) {
	aniDB, aniMock := newMockDB(t)
	defer aniDB.Close()
	malDB, malMock := newMockDB(t)
	defer malDB.Close()
	defer func() {
		if err := malMock.ExpectationsWereMet(); err != nil {
			t.Fatalf("unexpected MyAnimeList expectations: %v", err)
		}
	}()

	srv := &server{anilistDB: aniDB, myAnimeListDB: malDB}

	aniMock.ExpectQuery(`SELECT\s+COUNT\(\*\)\s+FROM\s+media\s+WHERE\s+title_romaji ILIKE \$1\s+AND\s+title_english ILIKE \$2\s+AND\s+title_native ILIKE \$3`).
		WithArgs("%romaji%", "%english%", "%native%").
		WillReturnRows(sqlmock.NewRows([]string{"count"}).AddRow(0))

	now := time.Now().UTC()
	columns := []string{
		"id", "type", "title_romaji", "title_english", "title_native",
		"synonyms", "description", "format", "status",
		"episodes", "duration", "country_of_origin", "source", "season",
		"season_year", "average_score", "mean_score", "popularity", "favourites",
		"genres", "tags", "studios",
		"start_date_year", "start_date_month", "start_date_day",
		"end_date_year", "end_date_month", "end_date_day",
		"cover_image", "banner_image", "updated_at", "site_url",
		"is_adult", "is_licensed",
	}
	rows := sqlmock.NewRows(columns).AddRow(
		2, "MANGA", "Romaji Title", "English Title", "Native Title",
		pq.StringArray{"Alt Title"}, "Another Description", "NOVEL", "FINISHED",
		12, 45, "JP", "ORIGINAL", "FALL",
		2021, 88, 80, 50000, 3000,
		pq.StringArray{"Action"}, []byte(`[{"id":3}]`), []byte(`[{"id":4}]`),
		2021, 9, 15,
		2022, 1, 1,
		"cover2.jpg", "banner2.jpg", now, "https://example.org",
		false, true,
	)

	aniMock.ExpectQuery(`(?s)SELECT\s+id.*FROM\s+media\s+WHERE\s+title_romaji ILIKE \$1\s+AND\s+title_english ILIKE \$2\s+AND\s+title_native ILIKE \$3\s+ORDER BY id LIMIT \$4 OFFSET \$5`).
		WithArgs("%romaji%", "%english%", "%native%", 20, 0).
		WillReturnRows(rows)

	req := httptest.NewRequest(http.MethodGet, "/anilist/media?title_romaji=romaji&title_english=english&title_native=native", nil)
	rec := httptest.NewRecorder()
	srv.routes().ServeHTTP(rec, req)

	if rec.Code != http.StatusOK {
		t.Fatalf("expected status 200, got %d: %s", rec.Code, rec.Body.String())
	}

	if err := aniMock.ExpectationsWereMet(); err != nil {
		t.Fatalf("unexpected AniList expectations: %v", err)
	}
}

func TestAniListMediaGetNotFound(t *testing.T) {
	aniDB, aniMock := newMockDB(t)
	defer aniDB.Close()
	malDB, malMock := newMockDB(t)
	defer malDB.Close()
	defer func() {
		if err := malMock.ExpectationsWereMet(); err != nil {
			t.Fatalf("unexpected MyAnimeList expectations: %v", err)
		}
	}()

	srv := &server{anilistDB: aniDB, myAnimeListDB: malDB}

	aniMock.ExpectQuery(`(?s)SELECT\s+id.*FROM\s+media.*WHERE id = \$1`).
		WithArgs(999).
		WillReturnError(sql.ErrNoRows)

	req := httptest.NewRequest(http.MethodGet, "/anilist/media/999", nil)
	rec := httptest.NewRecorder()
	srv.routes().ServeHTTP(rec, req)

	if rec.Code != http.StatusNotFound {
		t.Fatalf("expected status 404, got %d", rec.Code)
	}
	if err := aniMock.ExpectationsWereMet(); err != nil {
		t.Fatalf("unexpected AniList expectations: %v", err)
	}
}

func TestMyAnimeListAnimeList(t *testing.T) {
	aniDB, aniMock := newMockDB(t)
	defer aniDB.Close()
	malDB, malMock := newMockDB(t)
	defer malDB.Close()
	defer func() {
		if err := aniMock.ExpectationsWereMet(); err != nil {
			t.Fatalf("unexpected AniList expectations: %v", err)
		}
	}()

	srv := &server{anilistDB: aniDB, myAnimeListDB: malDB}

	malMock.ExpectQuery(`SELECT\s+COUNT\(\*\)\s+FROM\s+anime`).
		WillReturnRows(sqlmock.NewRows([]string{"count"}).AddRow(1))

	now := time.Now().UTC()
	columns := []string{
		"mal_id", "title", "title_english", "title_japanese",
		"type", "source", "episodes", "status", "airing",
		"aired_from", "aired_to", "duration", "rating",
		"score", "scored_by", "rank", "popularity", "members",
		"favorites", "synopsis", "background", "season", "year",
		"broadcast", "titles", "images", "trailer", "producers",
		"licensors", "studios", "genres", "themes", "demographics", "raw",
	}
	rows := sqlmock.NewRows(columns).AddRow(
		10, "Title", "Title EN", "Title JP",
		"TV", "Manga", 12, "Finished", true,
		now, now, "24m", "PG-13",
		8.7, 1200, 5, 42, 10000,
		500, "Synopsis", "Background", "spring", 2021,
		[]byte(`{"day":"monday"}`), // broadcast
		[]byte(`[{}]`),             // titles
		[]byte(`{"jpg":{}}`),       // images
		[]byte(`{"id":1}`),         // trailer
		[]byte(`[{}]`),             // producers
		[]byte(`[{}]`),             // licensors
		[]byte(`[{}]`),             // studios
		[]byte(`[{}]`),             // genres
		[]byte(`[{}]`),             // themes
		[]byte(`[{}]`),             // demographics
		[]byte(`{}`),               // raw
	)

	malMock.ExpectQuery(`(?s)SELECT\s+mal_id.*FROM\s+anime.*ORDER BY mal_id LIMIT \$1 OFFSET \$2`).
		WithArgs(20, 0).
		WillReturnRows(rows)

	req := httptest.NewRequest(http.MethodGet, "/myanimelist/anime", nil)
	rec := httptest.NewRecorder()
	srv.routes().ServeHTTP(rec, req)

	if rec.Code != http.StatusOK {
		t.Fatalf("expected status 200, got %d: %s", rec.Code, rec.Body.String())
	}

	var payload listResponse[myAnimeListAnime]
	if err := json.NewDecoder(rec.Body).Decode(&payload); err != nil {
		t.Fatalf("decode: %v", err)
	}
	if payload.Pagination.Total != 1 {
		t.Fatalf("expected total 1, got %d", payload.Pagination.Total)
	}
	if len(payload.Data) != 1 || payload.Data[0].ID != 10 {
		t.Fatalf("unexpected payload: %#v", payload.Data)
	}

	if err := malMock.ExpectationsWereMet(); err != nil {
		t.Fatalf("unexpected MyAnimeList expectations: %v", err)
	}
}

func TestMyAnimeListGetNotFound(t *testing.T) {
	aniDB, aniMock := newMockDB(t)
	defer aniDB.Close()
	malDB, malMock := newMockDB(t)
	defer malDB.Close()

	srv := &server{anilistDB: aniDB, myAnimeListDB: malDB}

	malMock.ExpectQuery(`(?s)SELECT\s+mal_id.*FROM\s+anime.*WHERE mal_id = \$1`).
		WithArgs(12345).
		WillReturnError(sql.ErrNoRows)

	req := httptest.NewRequest(http.MethodGet, "/myanimelist/anime/12345", nil)
	rec := httptest.NewRecorder()
	srv.routes().ServeHTTP(rec, req)

	if rec.Code != http.StatusNotFound {
		t.Fatalf("expected status 404, got %d", rec.Code)
	}

	if err := aniMock.ExpectationsWereMet(); err != nil {
		t.Fatalf("unexpected AniList expectations: %v", err)
	}
	if err := malMock.ExpectationsWereMet(); err != nil {
		t.Fatalf("unexpected MyAnimeList expectations: %v", err)
	}
}
