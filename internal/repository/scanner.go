package repository

import (
	"database/sql"
	"encoding/json"
	"strings"
)

func ScanNullString(ns sql.NullString) string {
	if ns.Valid {
		return ns.String
	}
	return ""
}

func ScanNullInt(ni sql.NullInt64) *int {
	if ni.Valid {
		v := int(ni.Int64)
		return &v
	}
	return nil
}

func ScanNullFloat(nf sql.NullFloat64) *float64 {
	if nf.Valid {
		v := nf.Float64
		return &v
	}
	return nil
}

func ScanNullBool(nb sql.NullBool) *bool {
	if nb.Valid {
		return &nb.Bool
	}
	return nil
}

func ScanNullTime(nt sql.NullTime) *interface{} {
	if nt.Valid {
		t := nt.Time.UTC()
		var result interface{} = &t
		return &result
	}
	return nil
}

func ScanJSONField(value []byte) json.RawMessage {
	if len(value) == 0 {
		return nil
	}
	trimmed := strings.TrimSpace(string(value))
	if trimmed == "" || strings.EqualFold(trimmed, "null") {
		return nil
	}
	return json.RawMessage([]byte(trimmed))
}

func ScanStringArray(value []byte) []string {
	if len(value) == 0 {
		return nil
	}
	var arr []string
	if err := json.Unmarshal(value, &arr); err != nil {
		return nil
	}
	return arr
}

func ScanPartialDate(year, month, day sql.NullInt64) (yearPtr, monthPtr, dayPtr *int) {
	if year.Valid {
		y := int(year.Int64)
		yearPtr = &y
	}
	if month.Valid {
		m := int(month.Int64)
		monthPtr = &m
	}
	if day.Valid {
		d := int(day.Int64)
		dayPtr = &d
	}
	return
}

