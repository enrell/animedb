package service

import (
	"context"

	"animedb/internal/repository"
)

type MyAnimeListSearchResult struct {
	ID            int
	Title         string
	TitleEnglish  string
	TitleJapanese string
	Score         float64
}

func HandleImprovedMyAnimeListSearch(ctx context.Context, repo repository.MyAnimeListRepository, search string, k int) ([]MyAnimeListSearchResult, error) {
	if k <= 0 {
		k = 1
	}

	prefiltered, err := repo.PrefilterAnime(ctx, search, 100)
	if err != nil {
		return nil, err
	}

	var candidates []*Document
	for _, result := range prefiltered {
		combinedTitle := result.Title.String + " " + result.TitleEnglish.String + " " + result.TitleJapanese.String

		tokens := tokenize(combinedTitle)
		ngramTokens := generateAllNGrams(tokens, 3)

		doc := &Document{
			ID:           result.ID,
			Text:         combinedTitle,
			Tokens:       ngramTokens,
			TitleRomaji:  result.Title.String,
			TitleEnglish: result.TitleEnglish.String,
			TitleNative:  result.TitleJapanese.String,
			SeasonNumber: 0,
		}

		candidates = append(candidates, doc)
	}

	if len(candidates) == 0 {
		return []MyAnimeListSearchResult{}, nil
	}

	engine := NewBM25SearchEngine()
	topDocs := engine.RankTopK(search, candidates, 0, false, k)

	if len(topDocs) == 0 {
		return []MyAnimeListSearchResult{}, nil
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

	return resultList, nil
}
