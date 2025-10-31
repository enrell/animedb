package anilist

import "fmt"

type PageResponse struct {
	Data struct {
		Page struct {
			PageInfo PageInfo   `json:"pageInfo"`
			Media    []MediaDTO `json:"media"`
		} `json:"Page"`
	} `json:"data"`
	Errors []GraphQLError `json:"errors"`
}

type GraphQLError struct {
	Message string `json:"message"`
	Status  int    `json:"status"`
}

type PageInfo struct {
	Total       int  `json:"total"`
	PerPage     int  `json:"perPage"`
	CurrentPage int  `json:"currentPage"`
	HasNextPage bool `json:"hasNextPage"`
}

type MediaDTO struct {
	ID              int           `json:"id"`
	Type            string        `json:"type"`
	Title           MediaTitle    `json:"title"`
	Synonyms        []string      `json:"synonyms"`
	Description     string        `json:"description"`
	Format          string        `json:"format"`
	Status          string        `json:"status"`
	Episodes        *int          `json:"episodes"`
	Duration        *int          `json:"duration"`
	CountryOfOrigin string        `json:"countryOfOrigin"`
	Source          string        `json:"source"`
	Season          string        `json:"season"`
	SeasonYear      *int          `json:"seasonYear"`
	AverageScore    *int          `json:"averageScore"`
	MeanScore       *int          `json:"meanScore"`
	Popularity      *int          `json:"popularity"`
	Favourites      *int          `json:"favourites"`
	Genres          []string      `json:"genres"`
	Tags            []MediaTag    `json:"tags"`
	StartDate       FuzzyDate     `json:"startDate"`
	EndDate         FuzzyDate     `json:"endDate"`
	CoverImage      CoverImage    `json:"coverImage"`
	BannerImage     string        `json:"bannerImage"`
	UpdatedAt       *int64        `json:"updatedAt"`
	SiteURL         string        `json:"siteUrl"`
	IsAdult         bool          `json:"isAdult"`
	IsLicensed      *bool         `json:"isLicensed"`
	Studios         StudioPayload `json:"studios"`
}

type MediaTitle struct {
	Romaji  string `json:"romaji"`
	English string `json:"english"`
	Native  string `json:"native"`
}

type MediaTag struct {
	ID               int    `json:"id"`
	Name             string `json:"name"`
	Rank             *int   `json:"rank"`
	IsMediaSpoiler   bool   `json:"isMediaSpoiler"`
	IsGeneralSpoiler bool   `json:"isGeneralSpoiler"`
	Description      string `json:"description"`
}

type FuzzyDate struct {
	Year  *int `json:"year"`
	Month *int `json:"month"`
	Day   *int `json:"day"`
}

type CoverImage struct {
	Large string `json:"large"`
}

type StudioPayload struct {
	Edges []struct {
		IsMain bool `json:"isMain"`
	} `json:"edges"`
	Nodes []struct {
		ID                int    `json:"id"`
		Name              string `json:"name"`
		SiteURL           string `json:"siteUrl"`
		IsAnimationStudio bool   `json:"isAnimationStudio"`
	} `json:"nodes"`
}

func GraphQLErrorMessages(errs []GraphQLError) []string {
	out := make([]string, 0, len(errs))
	for _, e := range errs {
		if e.Status != 0 {
			out = append(out, fmt.Sprintf("%s (status %d)", e.Message, e.Status))
		} else {
			out = append(out, e.Message)
		}
	}
	return out
}

