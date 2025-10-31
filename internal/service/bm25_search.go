package service

import (
	"context"
	"math"
	"regexp"
	"sort"
	"strings"
	"sync"
	"unicode"

	"golang.org/x/text/runes"
	"golang.org/x/text/transform"
	"golang.org/x/text/unicode/norm"
)

type BM25SearchEngine struct {
	k1           float64
	b            float64
	avgDocLength float64
	idfCache     map[string]float64
	mu           sync.RWMutex
}

type Document struct {
	ID           int
	Text         string
	Tokens       []string
	TitleRomaji  string
	TitleEnglish string
	TitleNative  string
	SeasonNumber int
	Score        float64
	Source       string
}

func NewBM25SearchEngine() *BM25SearchEngine {
	return &BM25SearchEngine{
		k1:       1.5,
		b:        0.75,
		idfCache: make(map[string]float64),
	}
}

func normalizeText(text string) string {
	t := transform.Chain(norm.NFD, runes.Remove(runes.In(unicode.Mn)), norm.NFC)
	result, _, _ := transform.String(t, text)

	result = strings.ToLower(result)
	reg := regexp.MustCompile(`[^a-z0-9\s]+`)
	result = reg.ReplaceAllString(result, " ")

	result = strings.Join(strings.Fields(result), " ")

	return strings.TrimSpace(result)
}

func tokenize(text string) []string {
	normalized := normalizeText(text)
	tokens := strings.Fields(normalized)
	return tokens
}

func TokenizePublic(text string) []string {
	return tokenize(text)
}

func generateNGrams(tokens []string, n int) []string {
	if len(tokens) < n {
		return []string{strings.Join(tokens, " ")}
	}

	ngrams := make([]string, 0, len(tokens)-n+1)
	for i := 0; i <= len(tokens)-n; i++ {
		ngram := strings.Join(tokens[i:i+n], " ")
		ngrams = append(ngrams, ngram)
	}
	return ngrams
}

func generateAllNGrams(tokens []string, maxN int) []string {
	var allNGrams []string

	allNGrams = append(allNGrams, tokens...)

	for n := 2; n <= maxN && n <= len(tokens); n++ {
		ngrams := generateNGrams(tokens, n)
		allNGrams = append(allNGrams, ngrams...)
	}

	return allNGrams
}

func GenerateAllNGramsPublic(tokens []string, maxN int) []string {
	return generateAllNGrams(tokens, maxN)
}

func (e *BM25SearchEngine) calculateBM25IDF(documents []*Document) {
	e.mu.Lock()
	defer e.mu.Unlock()

	totalDocs := float64(len(documents))
	docFreq := make(map[string]int)

	for _, doc := range documents {
		seen := make(map[string]bool)
		for _, token := range doc.Tokens {
			if !seen[token] {
				docFreq[token]++
				seen[token] = true
			}
		}
	}

	for term, df := range docFreq {
		e.idfCache[term] = math.Log((totalDocs-float64(df)+0.5)/(float64(df)+0.5) + 1)
	}
}

func (e *BM25SearchEngine) calculateBM25Score(queryTokens []string, doc *Document) float64 {
	e.mu.RLock()
	defer e.mu.RUnlock()

	docTF := make(map[string]int)
	for _, token := range doc.Tokens {
		docTF[token]++
	}

	docLength := float64(len(doc.Tokens))

	var score float64

	for _, qToken := range queryTokens {
		tf := float64(docTF[qToken])
		idf := e.idfCache[qToken]

		if idf == 0 {
			idf = 1.0
		}

		numerator := tf * (e.k1 + 1)
		denominator := tf + e.k1*(1-e.b+e.b*(docLength/e.avgDocLength))

		score += idf * (numerator / denominator)
	}

	return score
}

func (e *BM25SearchEngine) RankCandidates(ctx context.Context, query string, candidates []*Document, querySeason int, hasQuerySeason bool) *Document {
	if len(candidates) == 0 {
		return nil
	}

	topK := e.RankTopK(query, candidates, querySeason, hasQuerySeason, 1)
	if len(topK) == 0 {
		return nil
	}
	return topK[0]
}

func (e *BM25SearchEngine) RankTopK(query string, candidates []*Document, querySeason int, hasQuerySeason bool, k int) []*Document {
	if len(candidates) == 0 {
		return []*Document{}
	}

	if k <= 0 {
		k = 1
	}

	var totalLength float64
	for _, doc := range candidates {
		totalLength += float64(len(doc.Tokens))
	}
	e.avgDocLength = totalLength / float64(len(candidates))

	e.calculateBM25IDF(candidates)

	queryTokens := tokenize(query)
	queryNGrams := generateAllNGrams(queryTokens, 3)

	type rankedDoc struct {
		doc   *Document
		score float64
	}

	var ranked []rankedDoc

	for _, doc := range candidates {
		bm25Score := e.calculateBM25Score(queryNGrams, doc)
		score := bm25Score / 10.0

		if hasQuerySeason && doc.SeasonNumber > 0 {
			if querySeason == doc.SeasonNumber {
				score += 0.4
			} else {
				score -= 0.3
			}
		}

		ranked = append(ranked, rankedDoc{doc: doc, score: score})
	}

	sort.Slice(ranked, func(i, j int) bool {
		return ranked[i].score > ranked[j].score
	})

	var result []*Document
	for i, rd := range ranked {
		if i >= k {
			break
		}
		rd.doc.Score = rd.score
		result = append(result, rd.doc)
	}

	return result
}

