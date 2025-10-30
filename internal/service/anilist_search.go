package service

import (
	"context"
	"database/sql"
	"fmt"

	"animedb/internal/model"
	"animedb/internal/util"
)

func HandleImprovedAniListSearch(ctx context.Context, db *sql.DB, search string) ([]model.SearchResultWithMetadata, error) {
	querySeason, hasQuerySeason := util.ExtractSeasonNumber(search)
	baseQuery := util.RemoveSeasonFromQuery(search)

	searchTerm := baseQuery
	if !hasQuerySeason {
		searchTerm = search
	}

	const query = `
SELECT
	id,
	title_romaji,
	title_english,
	title_native,
	similarity(normalized_title, normalize_title($1)) AS score
FROM media
WHERE (
	length(normalize_title($1)) < 3
		AND normalized_title ILIKE '%' || normalize_title($1) || '%'
) OR normalized_title % normalize_title($1)
ORDER BY score DESC, id
LIMIT 20; -- Get more results for filtering
`

	rows, err := db.QueryContext(ctx, query, searchTerm)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var results []model.SearchResultWithMetadata
	for rows.Next() {
		var r model.SearchResultWithMetadata
		if err := rows.Scan(&r.ID, &r.TitleRomaji, &r.TitleEnglish, &r.TitleNative, &r.Score); err != nil {
			return nil, err
		}

		combinedTitle := fmt.Sprintf("%s %s %s",
			r.TitleRomaji.String, r.TitleEnglish.String, r.TitleNative.String)
		resultSeason, hasResultSeason := util.ExtractSeasonNumber(combinedTitle)

		if hasResultSeason {
			r.SeasonNumber = resultSeason
		}

		if hasQuerySeason && hasResultSeason {
			r.HasSeasonMatch = (querySeason == resultSeason)

			if r.HasSeasonMatch {
				r.Score += 0.3
			} else {
				r.Score -= 0.2
			}
		} else if hasQuerySeason && !hasResultSeason {
			r.Score -= 0.1
		}

		results = append(results, r)
	}

	if err := rows.Err(); err != nil {
		return nil, err
	}

	util.SortByScore(results)

	if len(results) > 5 {
		results = results[:5]
	}

	return results, nil
}
