package anilist

import (
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net/http"
	"time"

	"animedb/internal/ratelimit"
	"animedb/internal/util"
)

const (
	AniListEndpoint = "https://graphql.anilist.co"
	GraphQLQuery    = `
query ($page: Int, $perPage: Int, $type: MediaType, $sort: [MediaSort]) {
  Page(page: $page, perPage: $perPage) {
    pageInfo {
      total
      perPage
      currentPage
      hasNextPage
    }
    media(type: $type, sort: $sort) {
      id
      type
      title {
        romaji
        english
        native
      }
      synonyms
      description
      format
      status
      episodes
      duration
      countryOfOrigin
      source
      season
      seasonYear
      averageScore
      meanScore
      popularity
      favourites
      genres
      tags {
        id
        name
        rank
        isMediaSpoiler
        isGeneralSpoiler
        description
      }
      startDate {
        year
        month
        day
      }
      endDate {
        year
        month
        day
      }
      coverImage {
        large
      }
      bannerImage
      updatedAt
      siteUrl
      isAdult
      isLicensed
      studios {
        edges {
          isMain
        }
        nodes {
          id
          name
          siteUrl
          isAnimationStudio
        }
      }
    }
  }
}
`
)

type Client struct {
	httpClient *http.Client
	limiter    *ratelimit.Controller
}

func NewClient() *Client {
	return &Client{
		httpClient: &http.Client{
			Timeout: 30 * time.Second,
		},
		limiter: ratelimit.NewController(90),
	}
}

func (c *Client) FetchPage(ctx context.Context, page, perPage int, mediaType string, sort []string) (PageResponse, error) {
	var payload PageResponse
	if perPage <= 0 {
		perPage = 50
	}
	if perPage > 50 {
		perPage = 50
	}

	if len(sort) == 0 {
		sort = []string{"ID"}
	}

	body, err := json.Marshal(map[string]any{
		"query": GraphQLQuery,
		"variables": map[string]any{
			"page":    page,
			"perPage": perPage,
			"type":    mediaType,
			"sort":    sort,
		},
	})
	if err != nil {
		return payload, fmt.Errorf("encode request: %w", err)
	}

	for attempt := 0; attempt < 6; attempt++ {
		if err := c.limiter.Wait(ctx); err != nil {
			return payload, err
		}

		req, err := http.NewRequestWithContext(ctx, http.MethodPost, AniListEndpoint, bytes.NewBuffer(body))
		if err != nil {
			return payload, fmt.Errorf("build request: %w", err)
		}
		req.Header.Set("Content-Type", "application/json")
		req.Header.Set("Accept", "application/json")

		resp, err := c.httpClient.Do(req)
		if err != nil {
			if ctx.Err() != nil {
				return payload, ctx.Err()
			}
			time.Sleep(time.Duration(attempt+1) * 500 * time.Millisecond)
			continue
		}

		sleepDuration := c.limiter.AdjustFromResponse(resp)

		if resp.StatusCode == http.StatusTooManyRequests {
			retryAfter := util.ParseRetryAfter(resp.Header, time.Minute)
			resp.Body.Close()
			if err := util.SleepContext(ctx, retryAfter); err != nil {
				return payload, err
			}
			continue
		}

		if resp.StatusCode >= 300 {
			defer resp.Body.Close()
			data, _ := io.ReadAll(resp.Body)
			return payload, fmt.Errorf("anilist request failed: status=%d body=%s", resp.StatusCode, util.Truncate(string(data), 512))
		}

		respBytes, err := io.ReadAll(resp.Body)
		resp.Body.Close()
		if err != nil {
			return payload, fmt.Errorf("read response: %w", err)
		}

		if err := json.Unmarshal(respBytes, &payload); err != nil {
			return payload, fmt.Errorf("decode response: %w", err)
		}
		if len(payload.Errors) > 0 {
			return payload, fmt.Errorf("graphql errors: %v", GraphQLErrorMessages(payload.Errors))
		}

		if sleepDuration > 0 {
			if err := util.SleepContext(ctx, sleepDuration); err != nil {
				return payload, err
			}
		}

		return payload, nil
	}

	return payload, errors.New("exhausted retries fetching AniList data")
}
