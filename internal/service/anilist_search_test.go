package service

import (
	"context"
	"database/sql"
	"testing"
	"time"

	"animedb/internal/repository"

	"github.com/DATA-DOG/go-sqlmock"
)

func setupAniListRepo(t *testing.T) (repository.AniListRepository, sqlmock.Sqlmock, *sql.DB) {
	db, mock, err := sqlmock.New(sqlmock.QueryMatcherOption(sqlmock.QueryMatcherRegexp))
	if err != nil {
		t.Fatalf("failed to create sqlmock: %v", err)
	}

	repo := repository.NewAniListRepository(db)
	return repo, mock, db
}

func TestHandleImprovedAniListSearch(t *testing.T) {
	repo, mock, db := setupAniListRepo(t)
	defer db.Close()

	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()

	mock.ExpectQuery(`SELECT.*id.*title_romaji.*title_english.*title_native`).
		WithArgs("slime", 100).
		WillReturnRows(sqlmock.NewRows([]string{"id", "title_romaji", "title_english", "title_native"}).
			AddRow(1, "Tensei Shitara Slime Datta Ken", "That Time I Got Reincarnated as a Slime", "").
			AddRow(2, "Slime Taoshite", "Slime Hunting", ""))

	results, total, err := HandleImprovedAniListSearch(ctx, repo, "slime", 10)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if total < 0 {
		t.Error("expected non-negative total")
	}

	if len(results) == 0 {
		t.Error("expected at least one result")
	}

	if err := mock.ExpectationsWereMet(); err != nil {
		t.Errorf("unmet expectations: %v", err)
	}
}

func TestHandleImprovedAniListSearch_SeasonAware(t *testing.T) {
	repo, mock, db := setupAniListRepo(t)
	defer db.Close()

	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()

	mock.ExpectQuery(`SELECT.*id.*title_romaji.*title_english.*title_native`).
		WithArgs("slime", 100).
		WillReturnRows(sqlmock.NewRows([]string{"id", "title_romaji", "title_english", "title_native"}).
			AddRow(1, "Slime Season 1", "Slime Season 1", "").
			AddRow(2, "Slime Season 2", "Slime Season 2", ""))

	results, _, err := HandleImprovedAniListSearch(ctx, repo, "slime season 2", 10)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	if len(results) == 0 {
		t.Fatal("expected at least one result")
	}

	hasSeasonMatch := false
	for _, r := range results {
		if r.HasSeasonMatch && r.SeasonNumber == 2 {
			hasSeasonMatch = true
			break
		}
	}

	if !hasSeasonMatch {
		t.Error("expected season match for season 2")
	}

	if err := mock.ExpectationsWereMet(); err != nil {
		t.Errorf("unmet expectations: %v", err)
	}
}

func TestHandleImprovedAniListSearch_EmptyResult(t *testing.T) {
	repo, mock, db := setupAniListRepo(t)
	defer db.Close()

	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()

	mock.ExpectQuery(`SELECT.*id.*title_romaji.*title_english.*title_native`).
		WithArgs("nonexistent", 100).
		WillReturnRows(sqlmock.NewRows([]string{"id", "title_romaji", "title_english", "title_native"}))

	results, total, err := HandleImprovedAniListSearch(ctx, repo, "nonexistent", 10)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	if len(results) != 0 {
		t.Errorf("expected empty results, got %d", len(results))
	}
	if total != 0 {
		t.Errorf("expected total 0 for empty results, got %d", total)
	}

	if err := mock.ExpectationsWereMet(); err != nil {
		t.Errorf("unmet expectations: %v", err)
	}
}

func TestHandleImprovedAniListSearch_InvalidLimit(t *testing.T) {
	repo, mock, db := setupAniListRepo(t)
	defer db.Close()

	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()

	mock.ExpectQuery(`SELECT.*id.*title_romaji.*title_english.*title_native`).
		WithArgs("test", 100).
		WillReturnRows(sqlmock.NewRows([]string{"id", "title_romaji", "title_english", "title_native"}).
			AddRow(1, "Test", "Test", ""))

	results, total, err := HandleImprovedAniListSearch(ctx, repo, "test", 0)
	if total < 0 {
		t.Error("expected non-negative total")
	}
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	if len(results) != 1 {
		t.Errorf("expected 1 result with limit 0 (should default to 1), got %d", len(results))
	}

	if err := mock.ExpectationsWereMet(); err != nil {
		t.Errorf("unmet expectations: %v", err)
	}
}

