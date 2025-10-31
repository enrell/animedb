package response

import (
	"encoding/json"
	"fmt"
	"net/http"
	"strconv"
)

type PaginationMeta struct {
	Page     int  `json:"page"`
	PageSize int  `json:"page_size"`
	Total    int  `json:"total"`
	HasMore  bool `json:"has_more"`
}

type ListResponse[T any] struct {
	Data       []T            `json:"data"`
	Pagination PaginationMeta `json:"pagination"`
}

type SearchResponse[T any] struct {
	Data       []T            `json:"data"`
	Pagination PaginationMeta `json:"pagination"`
}

func WriteJSON(w http.ResponseWriter, status int, payload any) {
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(status)
	if payload == nil {
		return
	}
	if err := json.NewEncoder(w).Encode(payload); err != nil {
		fmt.Printf("write json error: %v\n", err)
	}
}

func WriteError(w http.ResponseWriter, status int, err error) {
	WriteJSON(w, status, map[string]string{
		"error": err.Error(),
	})
}

func ParsePagination(pageStr, sizeStr string, defaultSize, maxSize int) (int, int) {
	page := 1
	if p, err := strconv.Atoi(pageStr); err == nil && p > 0 {
		page = p
	}

	pageSize := defaultSize
	if s, err := strconv.Atoi(sizeStr); err == nil && s > 0 {
		pageSize = s
	}
	if pageSize > maxSize {
		pageSize = maxSize
	}
	return page, pageSize
}

