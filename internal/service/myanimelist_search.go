package service

import (
	"context"

	"animedb/internal/repository"
	"animedb/internal/util"
)

type MyAnimeListSearchResult struct {
	ID            int
	Title         string
	TitleEnglish  string
	TitleJapanese string
	Score         float64
}

func HandleImprovedMyAnimeListSearch(ctx context.Context, repo repository.MyAnimeListRepository, search string, k int) ([]MyAnimeListSearchResult, int, error) {
	if k <= 0 {
		k = 1
	}

	querySeason, hasQuerySeason := util.ExtractSeasonNumber(search)
	baseQuery := util.RemoveSeasonFromQuery(search)

	searchTerm := baseQuery
	if !hasQuerySeason {
		searchTerm = search
	}

	prefiltered, err := repo.PrefilterAnime(ctx, searchTerm, 100)
	if err != nil {
		return nil, 0, err
	}

	totalCandidates := len(prefiltered)

	var candidates []*Document
	for _, result := range prefiltered {
		combinedTitle := result.Title.String + " " + result.TitleEnglish.String + " " + result.TitleJapanese.String

		tokens := tokenize(combinedTitle)
		ngramTokens := generateAllNGrams(tokens, 3)

		season, _ := util.ExtractSeasonNumber(combinedTitle)

		doc := &Document{
			ID:           result.ID,
			Text:         combinedTitle,
			Tokens:       ngramTokens,
			TitleRomaji:  result.Title.String,
			TitleEnglish: result.TitleEnglish.String,
			TitleNative:  result.TitleJapanese.String,
			SeasonNumber: season,
		}

		candidates = append(candidates, doc)
	}

	if len(candidates) == 0 {
		return []MyAnimeListSearchResult{}, 0, nil
	}

	engine := NewBM25SearchEngine()
	topDocs := engine.RankTopK(search, candidates, querySeason, hasQuerySeason, k)

	if len(topDocs) == 0 {
		return []MyAnimeListSearchResult{}, totalCandidates, nil
	}

	var resultList []MyAnimeListSearchResult
	for _, doc := range topDocs {
		result := MyAnimeListSearchResult{
			ID:            doc.ID,
			Title:         doc.TitleRomaji,
			TitleEnglish:  doc.TitleEnglish,
			TitleJapanese: doc.TitleNative,
			Score:         doc.Score,
		}
		resultList = append(resultList, result)
	}

	return resultList, totalCandidates, nil
}
