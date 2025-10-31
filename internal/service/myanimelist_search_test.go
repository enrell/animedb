package service

import (
	"context"
	"database/sql"
	"testing"
	"time"

	"animedb/internal/repository"

	"github.com/DATA-DOG/go-sqlmock"
)

func setupMyAnimeListRepo(t *testing.T) (repository.MyAnimeListRepository, sqlmock.Sqlmock, *sql.DB) {
	db, mock, err := sqlmock.New(sqlmock.QueryMatcherOption(sqlmock.QueryMatcherRegexp))
	if err != nil {
		t.Fatalf("failed to create sqlmock: %v", err)
	}

	repo := repository.NewMyAnimeListRepository(db)
	return repo, mock, db
}

func TestHandleImprovedMyAnimeListSearch(t *testing.T) {
	repo, mock, db := setupMyAnimeListRepo(t)
	defer db.Close()

	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()

	mock.ExpectQuery(`SELECT.*mal_id.*title.*title_english.*title_japanese`).
		WithArgs("slime", 100).
		WillReturnRows(sqlmock.NewRows([]string{"mal_id", "title", "title_english", "title_japanese", "score"}).
			AddRow(1, "Tensei Shitara Slime Datta Ken", "That Time I Got Reincarnated as a Slime", "", 0.8).
			AddRow(2, "Slime Taoshite", "Slime Hunting", "", 0.7))

	results, err := HandleImprovedMyAnimeListSearch(ctx, repo, "slime", 10)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	if len(results) == 0 {
		t.Error("expected at least one result")
	}

	if err := mock.ExpectationsWereMet(); err != nil {
		t.Errorf("unmet expectations: %v", err)
	}
}

func TestHandleImprovedMyAnimeListSearch_SeasonAware(t *testing.T) {
	repo, mock, db := setupMyAnimeListRepo(t)
	defer db.Close()

	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()

	mock.ExpectQuery(`SELECT.*mal_id.*title.*title_english.*title_japanese`).
		WithArgs("attack", 100).
		WillReturnRows(sqlmock.NewRows([]string{"mal_id", "title", "title_english", "title_japanese", "score"}).
			AddRow(1, "Attack on Titan Season 1", "Attack on Titan Season 1", "", 0.8).
			AddRow(2, "Attack on Titan Season 2", "Attack on Titan Season 2", "", 0.9))

	results, err := HandleImprovedMyAnimeListSearch(ctx, repo, "attack season 2", 10)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	if len(results) == 0 {
		t.Fatal("expected at least one result")
	}

	if err := mock.ExpectationsWereMet(); err != nil {
		t.Errorf("unmet expectations: %v", err)
	}
}

func TestHandleImprovedMyAnimeListSearch_EmptyResult(t *testing.T) {
	repo, mock, db := setupMyAnimeListRepo(t)
	defer db.Close()

	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()

	mock.ExpectQuery(`SELECT.*mal_id.*title.*title_english.*title_japanese`).
		WithArgs("nonexistent", 100).
		WillReturnRows(sqlmock.NewRows([]string{"mal_id", "title", "title_english", "title_japanese", "score"}))

	results, err := HandleImprovedMyAnimeListSearch(ctx, repo, "nonexistent", 10)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	if len(results) != 0 {
		t.Errorf("expected empty results, got %d", len(results))
	}

	if err := mock.ExpectationsWereMet(); err != nil {
		t.Errorf("unmet expectations: %v", err)
	}
}

func TestHandleImprovedMyAnimeListSearch_InvalidLimit(t *testing.T) {
	repo, mock, db := setupMyAnimeListRepo(t)
	defer db.Close()

	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()

	mock.ExpectQuery(`SELECT.*mal_id.*title.*title_english.*title_japanese`).
		WithArgs("test", 100).
		WillReturnRows(sqlmock.NewRows([]string{"mal_id", "title", "title_english", "title_japanese", "score"}).
			AddRow(1, "Test", "Test", "", 0.5))

	results, err := HandleImprovedMyAnimeListSearch(ctx, repo, "test", 0)
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

