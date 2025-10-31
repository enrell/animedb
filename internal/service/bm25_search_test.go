package service

import (
	"testing"
)

func TestBM25SearchEngine_RankTopK(t *testing.T) {
	engine := NewBM25SearchEngine()

	candidates := []*Document{
		{
			ID:     1,
			Text:   "attack on titan",
			Tokens: []string{"attack", "on", "titan"},
		},
		{
			ID:     2,
			Text:   "one punch man",
			Tokens: []string{"one", "punch", "man"},
		},
		{
			ID:     3,
			Text:   "attack on titan season 2",
			Tokens: []string{"attack", "on", "titan", "season", "2"},
			SeasonNumber: 2,
		},
	}

	for _, doc := range candidates {
		doc.Tokens = GenerateAllNGramsPublic(doc.Tokens, 3)
	}

	results := engine.RankTopK("attack titan", candidates, 0, false, 2)

	if len(results) != 2 {
		t.Errorf("expected 2 results, got %d", len(results))
	}

	if results[0].ID != 1 && results[0].ID != 3 {
		t.Errorf("expected first result to be attack on titan related, got ID %d", results[0].ID)
	}
}

func TestBM25SearchEngine_SeasonAwareness(t *testing.T) {
	engine := NewBM25SearchEngine()

	candidates := []*Document{
		{
			ID:          1,
			Text:        "slime season 1",
			Tokens:      []string{"slime", "season", "1"},
			SeasonNumber: 1,
		},
		{
			ID:          2,
			Text:        "slime season 2",
			Tokens:      []string{"slime", "season", "2"},
			SeasonNumber: 2,
		},
		{
			ID:          3,
			Text:        "slime season 3",
			Tokens:      []string{"slime", "season", "3"},
			SeasonNumber: 3,
		},
	}

	for _, doc := range candidates {
		doc.Tokens = GenerateAllNGramsPublic(doc.Tokens, 3)
	}

	results := engine.RankTopK("slime season 2", candidates, 2, true, 3)

	if len(results) == 0 {
		t.Fatal("expected at least one result")
	}

	if results[0].SeasonNumber != 2 {
		t.Errorf("expected season 2 to be ranked first, got season %d", results[0].SeasonNumber)
	}

	if results[0].Score <= 0 {
		t.Error("expected positive score")
	}
}

func TestTokenizePublic(t *testing.T) {
	tests := []struct {
		name     string
		input    string
		expected []string
	}{
		{
			name:     "simple text",
			input:    "attack on titan",
			expected: []string{"attack", "on", "titan"},
		},
		{
			name:     "with punctuation",
			input:    "Attack-On-Titan!",
			expected: []string{"attack", "on", "titan"},
		},
		{
			name:     "empty string",
			input:    "",
			expected: []string{},
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result := TokenizePublic(tt.input)
			if len(result) != len(tt.expected) {
				t.Errorf("expected %d tokens, got %d", len(tt.expected), len(result))
			}
		})
	}
}

func TestGenerateAllNGramsPublic(t *testing.T) {
	tokens := []string{"attack", "on", "titan"}
	ngrams := GenerateAllNGramsPublic(tokens, 3)

	expectedMin := len(tokens)
	if len(ngrams) < expectedMin {
		t.Errorf("expected at least %d ngrams, got %d", expectedMin, len(ngrams))
	}

	containsUnigram := false
	containsBigram := false
	containsTrigram := false

	for _, ngram := range ngrams {
		if ngram == "attack" {
			containsUnigram = true
		}
		if ngram == "attack on" {
			containsBigram = true
		}
		if ngram == "attack on titan" {
			containsTrigram = true
		}
	}

	if !containsUnigram {
		t.Error("expected unigrams")
	}
	if !containsBigram {
		t.Error("expected bigrams")
	}
	if !containsTrigram {
		t.Error("expected trigrams")
	}
}

