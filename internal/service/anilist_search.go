package service

import (
	"context"
	"database/sql"
	"fmt"

	"animedb/internal/model"
	"animedb/internal/repository"
	"animedb/internal/util"
)

func HandleImprovedAniListSearch(ctx context.Context, repo repository.AniListRepository, search string, k int) ([]model.SearchResultWithMetadata, error) {
	if k <= 0 {
		k = 1
	}

	querySeason, hasQuerySeason := util.ExtractSeasonNumber(search)
	baseQuery := util.RemoveSeasonFromQuery(search)

	searchTerm := baseQuery
	if !hasQuerySeason {
		searchTerm = search
	}

	results, err := repo.PrefilterMedia(ctx, searchTerm, 100)
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
	topDocs := engine.RankTopK(search, candidates, querySeason, hasQuerySeason, k)

	if len(topDocs) == 0 {
		return []model.SearchResultWithMetadata{}, nil
	}

	var resultList []model.SearchResultWithMetadata
	for _, bestDoc := range topDocs {
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

		resultList = append(resultList, result)
	}

	return resultList, nil
}
