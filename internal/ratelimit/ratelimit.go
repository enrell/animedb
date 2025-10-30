package ratelimit

import (
	"context"
	"net/http"
	"strconv"
	"sync"
	"time"
)

// Controller keeps track of rate limit metadata based on server responses
// and enforces a conservative gap between requests.
type Controller struct {
	mu          sync.Mutex
	minInterval time.Duration
	lastRequest time.Time
}

func NewController(defaultLimitPerMinute int) *Controller {
	interval := time.Second
	if defaultLimitPerMinute > 0 {
		interval = time.Minute / time.Duration(defaultLimitPerMinute)
	}
	return &Controller{minInterval: interval}
}

// Wait blocks until enough time has elapsed between requests.
func (c *Controller) Wait(ctx context.Context) error {
	c.mu.Lock()
	defer c.mu.Unlock()

	if c.lastRequest.IsZero() {
		c.lastRequest = time.Now()
		return nil
	}

	wait := c.minInterval - time.Since(c.lastRequest)
	if wait <= 0 {
		c.lastRequest = time.Now()
		return nil
	}

	timer := time.NewTimer(wait)
	defer timer.Stop()

	select {
	case <-ctx.Done():
		return ctx.Err()
	case <-timer.C:
		c.lastRequest = time.Now()
		return nil
	}
}

// AdjustFromResponse inspects the headers and updates the internal pacing.
// It returns how long the caller should sleep before issuing the next request.
func (c *Controller) AdjustFromResponse(resp *http.Response) time.Duration {
	c.mu.Lock()
	defer c.mu.Unlock()

	if limit := parseInt(resp.Header.Get("X-RateLimit-Limit")); limit > 0 {
		interval := time.Minute / time.Duration(limit)
		if interval > 0 {
			c.minInterval = interval
		}
	}

	remaining := parseInt(resp.Header.Get("X-RateLimit-Remaining"))
	if remaining > 3 {
		return 0
	}

	if reset := parseInt(resp.Header.Get("X-RateLimit-Reset")); reset > 0 {
		resetTime := time.Unix(int64(reset), 0)
		sleep := time.Until(resetTime)
		if sleep > 0 {
			return sleep
		}
	}

	// When no explicit reset header is provided, fall back to a single interval.
	if c.minInterval > 0 {
		return c.minInterval
	}
	return time.Second
}

func parseInt(value string) int {
	if value == "" {
		return 0
	}
	n, err := strconv.Atoi(value)
	if err != nil {
		return 0
	}
	return n
}
