package service

import (
	"context"
	"database/sql"
	"fmt"

	"animedb/internal/model"
	"animedb/internal/repository"
	"animedb/internal/util"
)

func HandleImprovedAniListSearch(ctx context.Context, repo repository.AniListRepository, search string) ([]model.SearchResultWithMetadata, error) {
	querySeason, hasQuerySeason := util.ExtractSeasonNumber(search)
	baseQuery := util.RemoveSeasonFromQuery(search)

	searchTerm := baseQuery
	if !hasQuerySeason {
		searchTerm = search
	}

	results, err := repo.SearchMedia(ctx, searchTerm)
	if err != nil {
		return nil, err
	}

	var candidates []*Document
	for _, result := range results {
		combinedTitle := fmt.Sprintf("%s %s %s",
			result.TitleRomaji.String, result.TitleEnglish.String, result.TitleNative.String)

		tokens := tokenize(combinedTitle)
		ngramTokens := generateAllNGrams(tokens, 3)

		season, _ := util.ExtractSeasonNumber(combinedTitle)

		doc := &Document{
			ID:           result.ID,
			Text:         combinedTitle,
			Tokens:       ngramTokens,
			TitleRomaji:  result.TitleRomaji.String,
			TitleEnglish: result.TitleEnglish.String,
			TitleNative:  result.TitleNative.String,
			SeasonNumber: season,
		}

		candidates = append(candidates, doc)
	}

	if len(candidates) == 0 {
		return []model.SearchResultWithMetadata{}, nil
	}

	engine := NewBM25SearchEngine()
	bestDoc := engine.RankCandidates(ctx, search, candidates, querySeason, hasQuerySeason)

	if bestDoc == nil {
		return []model.SearchResultWithMetadata{}, nil
	}

	result := model.SearchResultWithMetadata{
		ID:           bestDoc.ID,
		TitleRomaji:  sql.NullString{String: bestDoc.TitleRomaji, Valid: bestDoc.TitleRomaji != ""},
		TitleEnglish: sql.NullString{String: bestDoc.TitleEnglish, Valid: bestDoc.TitleEnglish != ""},
		TitleNative:  sql.NullString{String: bestDoc.TitleNative, Valid: bestDoc.TitleNative != ""},
		SeasonNumber: bestDoc.SeasonNumber,
		Score:        1.0,
	}

	if hasQuerySeason && bestDoc.SeasonNumber > 0 {
		result.HasSeasonMatch = (querySeason == bestDoc.SeasonNumber)
	}

	return []model.SearchResultWithMetadata{result}, nil
}
