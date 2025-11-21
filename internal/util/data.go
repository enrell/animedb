package util

import (
	"database/sql"
	"encoding/json"
	"strings"
	"time"
)

func NullIfEmpty(s string) interface{} {
	if strings.TrimSpace(s) == "" {
		return nil
	}
	return s
}

func NullIntPointer(ptr *int) interface{} {
	if ptr == nil {
		return nil
	}
	return *ptr
}

func NullFloatPointer(ptr *float64) interface{} {
	if ptr == nil {
		return nil
	}
	return *ptr
}

func ValueOrZero(ptr *int) interface{} {
	if ptr == nil || *ptr == 0 {
		return nil
	}
	return *ptr
}

func NormalizeDescription(desc string) string {
	trimmed := strings.TrimSpace(desc)
	if trimmed == "" || trimmed == "null" {
		return ""
	}
	return trimmed
}

func EmptyJSONIfNil(raw json.RawMessage) interface{} {
	if len(raw) == 0 {
		return nil
	}
	trimmed := strings.TrimSpace(string(raw))
	if trimmed == "" || strings.EqualFold(trimmed, "null") {
		return nil
	}
	return json.RawMessage([]byte(trimmed))
}

func ParseRFC3339(value string) (sql.NullTime, error) {
	value = strings.TrimSpace(value)
	if value == "" || strings.EqualFold(value, "null") {
		return sql.NullTime{}, nil
	}
	t, err := time.Parse(time.RFC3339, value)
	if err != nil {
		return sql.NullTime{}, err
	}
	return sql.NullTime{Time: t.UTC(), Valid: true}, nil
}

