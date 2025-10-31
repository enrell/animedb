package myanimelist

import (
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

const JikanEndpoint = "https://api.jikan.moe/v4/anime"

type Client struct {
	httpClient *http.Client
	limiter    *ratelimit.Controller
}

func NewClient() *Client {
	return &Client{
		httpClient: &http.Client{Timeout: 30 * time.Second},
		limiter:    ratelimit.NewController(60),
	}
}

func (c *Client) FetchPage(ctx context.Context, page, perPage int) (JikanResponse, error) {
	var payload JikanResponse

	if perPage <= 0 {
		perPage = 25
	}
	if perPage > 25 {
		perPage = 25
	}

	query := fmt.Sprintf("%s?page=%d&limit=%d&order_by=mal_id&sort=asc", JikanEndpoint, page, perPage)

	for attempt := 0; attempt < 6; attempt++ {
		if err := c.limiter.Wait(ctx); err != nil {
			return payload, err
		}

		req, err := http.NewRequestWithContext(ctx, http.MethodGet, query, nil)
		if err != nil {
			return payload, fmt.Errorf("build request: %w", err)
		}
		req.Header.Set("Accept", "application/json")
		req.Header.Set("User-Agent", "animedb-ingestor/1.0")

		resp, err := c.httpClient.Do(req)
		if err != nil {
			if ctx.Err() != nil {
				return payload, ctx.Err()
			}
			time.Sleep(time.Duration(attempt+1) * 500 * time.Millisecond)
			continue
		}

		sleep := c.limiter.AdjustFromResponse(resp)

		if resp.StatusCode == http.StatusTooManyRequests {
			retry := util.ParseRetryAfter(resp.Header, 2*time.Second)
			resp.Body.Close()
			if err := util.SleepContext(ctx, retry); err != nil {
				return payload, err
			}
			continue
		}

		if resp.StatusCode >= 300 {
			defer resp.Body.Close()
			data, _ := io.ReadAll(resp.Body)
			return payload, fmt.Errorf("jikan request failed: status=%d body=%s", resp.StatusCode, util.Truncate(string(data), 512))
		}

		body, err := io.ReadAll(resp.Body)
		resp.Body.Close()
		if err != nil {
			return payload, fmt.Errorf("read response: %w", err)
		}

		if err := json.Unmarshal(body, &payload); err != nil {
			return payload, fmt.Errorf("decode response: %w", err)
		}

		if len(payload.Errors) > 0 {
			return payload, fmt.Errorf("jikan errors: %v", payload.Errors)
		}

		if sleep > 0 {
			if err := util.SleepContext(ctx, sleep); err != nil {
				return payload, err
			}
		}

		return payload, nil
	}

	return payload, errors.New("exhausted retries fetching Jikan data")
}

func DecodeAnime(rawItems []json.RawMessage) ([]JikanAnime, error) {
	result := make([]JikanAnime, 0, len(rawItems))
	for _, raw := range rawItems {
		var item JikanAnime
		if err := json.Unmarshal(raw, &item); err != nil {
			return nil, fmt.Errorf("unmarshal anime entry: %w", err)
		}
		item.Raw = raw
		result = append(result, item)
	}
	return result, nil
}

