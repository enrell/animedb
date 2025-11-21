package service

import (
	"context"
	"strings"

	"animedb/internal/model"
	"animedb/internal/repository"
)

type VideoSearchService struct {
	repo      repository.VideoRepository
	bm25Engine *BM25SearchEngine
}

func NewVideoSearchService(repo repository.VideoRepository) *VideoSearchService {
	return &VideoSearchService{
		repo:      repo,
		bm25Engine: NewBM25SearchEngine(),
	}
}

type VideoSearchResult struct {
	Anime   model.Anime
	Score   float64
	Matches []string
}

func (s *VideoSearchService) Search(ctx context.Context, query string, limit int) ([]VideoSearchResult, error) {
	if limit <= 0 || limit > 50 {
		limit = 10
	}

	candidates, err := s.repo.PrefilterAnime(ctx, query, 100)
	if err != nil {
		return nil, err
	}

	if len(candidates) == 0 {
		return []VideoSearchResult{}, nil
	}

	documents := make([]*Document, 0, len(candidates))
	for _, anime := range candidates {
		text := anime.Title
		tokens := TokenizePublic(text)
		doc := &Document{
			ID:     anime.ID,
			Text:   text,
			Tokens: tokens,
		}
		documents = append(documents, doc)
	}

	ranked := s.bm25Engine.RankTopK(query, documents, 0, false, 0, false, "", false, limit)

	results := make([]VideoSearchResult, 0, len(ranked))
	for _, doc := range ranked {
		anime := candidates[0]
		for _, a := range candidates {
			if a.ID == doc.ID {
				anime = a
				break
			}
		}

		matches := s.findMatches(query, anime.Title)
		results = append(results, VideoSearchResult{
			Anime:   anime,
			Score:   doc.Score,
			Matches: matches,
		})
	}

	return results, nil
}

func (s *VideoSearchService) findMatches(query, text string) []string {
	queryLower := strings.ToLower(query)
	textLower := strings.ToLower(text)
	queryWords := strings.Fields(queryLower)

	var matches []string
	for _, word := range queryWords {
		if strings.Contains(textLower, word) {
			idx := strings.Index(textLower, word)
			start := max(0, idx-10)
			end := min(len(text), idx+len(word)+10)
			match := text[start:end]
			if !contains(matches, match) {
				matches = append(matches, match)
			}
		}
	}
	return matches
}

func max(a, b int) int {
	if a > b {
		return a
	}
	return b
}

func min(a, b int) int {
	if a < b {
		return a
	}
	return b
}

func contains(slice []string, item string) bool {
	for _, s := range slice {
		if s == item {
			return true
		}
	}
	return false
}

