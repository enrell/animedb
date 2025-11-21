package middleware

import (
	"io"
	"net/http"
	"regexp"
	"strings"
)

const (
	maxRequestSize = 10 * 1024
	maxQueryLength = 200
	maxQueryParams = 50
)

var (
	sqlInjectionPattern = regexp.MustCompile(`(?i)(union|select|insert|update|delete|drop|create|alter|exec|execute|script|javascript|vbscript|onload|onerror)`)
	pathTraversalPattern = regexp.MustCompile(`\.\.[/\\]`)
)

func ValidationMiddleware(next http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.Method != http.MethodGet {
			if !validateRequestSize(r, w) {
				return
			}
		}

		if !validateQueryParams(r, w) {
			return
		}

		if !validateQueryValues(r, w) {
			return
		}

		next.ServeHTTP(w, r)
	})
}

func validateRequestSize(r *http.Request, w http.ResponseWriter) bool {
	r.Body = http.MaxBytesReader(w, r.Body, maxRequestSize)

	body, err := io.ReadAll(r.Body)
	if err != nil {
		http.Error(w, "Request body too large", http.StatusRequestEntityTooLarge)
		return false
	}
	r.Body = io.NopCloser(strings.NewReader(string(body)))

	return true
}

func validateQueryParams(r *http.Request, w http.ResponseWriter) bool {
	if len(r.URL.RawQuery) > maxQueryLength*maxQueryParams {
		http.Error(w, "Query string too long", http.StatusBadRequest)
		return false
	}

	params := r.URL.Query()
	if len(params) > maxQueryParams {
		http.Error(w, "Too many query parameters", http.StatusBadRequest)
		return false
	}

	return true
}

func validateQueryValues(r *http.Request, w http.ResponseWriter) bool {
	for key, values := range r.URL.Query() {
		if len(key) > maxQueryLength {
			http.Error(w, "Query parameter name too long", http.StatusBadRequest)
			return false
		}

		if strings.Contains(key, "..") || pathTraversalPattern.MatchString(key) {
			http.Error(w, "Invalid query parameter", http.StatusBadRequest)
			return false
		}

		for _, value := range values {
			if len(value) > maxQueryLength {
				http.Error(w, "Query parameter value too long", http.StatusBadRequest)
				return false
			}

			if sqlInjectionPattern.MatchString(value) {
				http.Error(w, "Invalid characters in query parameter", http.StatusBadRequest)
				return false
			}

			if pathTraversalPattern.MatchString(value) {
				http.Error(w, "Invalid characters in query parameter", http.StatusBadRequest)
				return false
			}
		}
	}

	return true
}

func SanitizeInput(input string) string {
	input = strings.TrimSpace(input)
	input = strings.ReplaceAll(input, "\x00", "")
	input = strings.ReplaceAll(input, "\r", "")
	input = strings.ReplaceAll(input, "\n", "")
	return input
}

