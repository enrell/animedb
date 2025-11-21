package service

import (
	"context"
	"database/sql"
	"fmt"

	"animedb/internal/model"
	"animedb/internal/repository"
	"animedb/internal/util"
)

func HandleImprovedAniListSearch(ctx context.Context, repo repository.AniListRepository, search string, format *string, k int) ([]model.SearchResultWithMetadata, int, error) {
	if k <= 0 {
		k = 1
	}

	querySeason, hasQuerySeason := util.ExtractSeasonNumber(search)
	queryPart, hasQueryPart := util.ExtractPartNumber(search)
	baseQuery := util.RemoveSeasonFromQuery(search)

	searchTerm := baseQuery
	if !hasQuerySeason {
		searchTerm = search
	}

	if searchTerm == "" {
		searchTerm = search
	}

	results, err := repo.PrefilterMedia(ctx, searchTerm, format, 100)
	if err != nil {
		return nil, 0, err
	}

	totalCandidates := len(results)

	var candidates []*Document
	for _, result := range results {
		combinedTitle := fmt.Sprintf("%s %s %s",
			result.TitleRomaji.String, result.TitleEnglish.String, result.TitleNative.String)

		tokens := tokenize(combinedTitle)
		ngramTokens := generateAllNGrams(tokens, 3)

		season, _ := util.ExtractSeasonNumber(combinedTitle)
		part, _ := util.ExtractPartNumber(combinedTitle)

		doc := &Document{
			ID:           result.ID,
			Text:         combinedTitle,
			Tokens:       ngramTokens,
			TitleRomaji:  result.TitleRomaji.String,
			TitleEnglish: result.TitleEnglish.String,
			TitleNative:  result.TitleNative.String,
			SeasonNumber: season,
			PartNumber:   part,
			Format:       result.Format.String,
			Type:         result.Type.String,
		}

		candidates = append(candidates, doc)
	}

	if len(candidates) == 0 {
		return []model.SearchResultWithMetadata{}, 0, nil
	}

	engine := NewBM25SearchEngine()
	hasQueryFormat := format != nil && *format != ""
	queryFormat := ""
	if hasQueryFormat {
		queryFormat = *format
	}
	topDocs := engine.RankTopK(search, candidates, querySeason, hasQuerySeason, queryPart, hasQueryPart, queryFormat, hasQueryFormat, k)

	if len(topDocs) == 0 {
		return []model.SearchResultWithMetadata{}, totalCandidates, nil
	}

	var resultList []model.SearchResultWithMetadata
	const minScoreThreshold = 0.05
	
	for _, bestDoc := range topDocs {
		if bestDoc.Score < minScoreThreshold {
			continue
		}
		
		result := model.SearchResultWithMetadata{
			ID:           bestDoc.ID,
			TitleRomaji:  sql.NullString{String: bestDoc.TitleRomaji, Valid: bestDoc.TitleRomaji != ""},
			TitleEnglish: sql.NullString{String: bestDoc.TitleEnglish, Valid: bestDoc.TitleEnglish != ""},
			TitleNative:  sql.NullString{String: bestDoc.TitleNative, Valid: bestDoc.TitleNative != ""},
			SeasonNumber: bestDoc.SeasonNumber,
			PartNumber:   bestDoc.PartNumber,
			Score:        bestDoc.Score,
		}

		if hasQuerySeason && bestDoc.SeasonNumber > 0 {
			result.HasSeasonMatch = (querySeason == bestDoc.SeasonNumber)
		}

		resultList = append(resultList, result)
	}

	return resultList, totalCandidates, nil
}
