use crate::model::{CanonicalMedia, SourceName};

#[derive(Debug, Clone)]
pub struct MergeDecision<T> {
    pub value: T,
    pub score: f64,
    pub reason: String,
}

pub fn provider_weight(source: SourceName) -> f64 {
    match source {
        SourceName::AniList => 0.90,
        SourceName::Jikan => 0.76,
        SourceName::MyAnimeList => 0.80,
        SourceName::Kitsu => 0.78,
    }
}

pub fn score_text_field(
    source: SourceName,
    field_name: &str,
    value: &str,
    candidate: &CanonicalMedia,
) -> MergeDecision<String> {
    let provider_score = provider_weight(source) * 0.25;
    let completeness_score = text_completeness(value) * 0.20;
    let field_quality_score = text_quality(field_name, value) * 0.35;
    let cover_score = 0.0;
    let consistency_score = consistency_bonus(candidate) * 0.20;
    let total = provider_score + completeness_score + field_quality_score + cover_score + consistency_score;

    MergeDecision {
        value: value.to_string(),
        score: total,
        reason: format!(
            "provider={provider_score:.3}, completeness={completeness_score:.3}, quality={field_quality_score:.3}, consistency={consistency_score:.3}"
        ),
    }
}

pub fn score_optional_i32(
    source: SourceName,
    value: i32,
    candidate: &CanonicalMedia,
) -> MergeDecision<i32> {
    let provider_score = provider_weight(source) * 0.25;
    let completeness_score = if value > 0 { 0.20 } else { 0.0 };
    let quality_score = if value > 1 { 0.30 } else { 0.10 };
    let consistency_score = consistency_bonus(candidate) * 0.25;
    let total = provider_score + completeness_score + quality_score + consistency_score;

    MergeDecision {
        value,
        score: total,
        reason: format!(
            "provider={provider_score:.3}, completeness={completeness_score:.3}, numeric_quality={quality_score:.3}, consistency={consistency_score:.3}"
        ),
    }
}

pub fn score_cover_image(
    source: SourceName,
    value: &str,
    candidate: &CanonicalMedia,
) -> MergeDecision<String> {
    let provider_score = provider_weight(source) * 0.20;
    let completeness_score = text_completeness(value) * 0.10;
    let cover_quality_score = cover_quality(value) * 0.45;
    let consistency_score = consistency_bonus(candidate) * 0.15;
    let freshness_score = candidate.provider_rating.unwrap_or_default().clamp(0.0, 1.0) * 0.10;
    let total =
        provider_score + completeness_score + cover_quality_score + consistency_score + freshness_score;

    MergeDecision {
        value: value.to_string(),
        score: total,
        reason: format!(
            "provider={provider_score:.3}, completeness={completeness_score:.3}, cover_quality={cover_quality_score:.3}, consistency={consistency_score:.3}, freshness={freshness_score:.3}"
        ),
    }
}

pub fn score_boolean(source: SourceName, value: bool, candidate: &CanonicalMedia) -> MergeDecision<bool> {
    let provider_score = provider_weight(source) * 0.30;
    let value_score = if value { 0.35 } else { 0.20 };
    let consistency_score = consistency_bonus(candidate) * 0.35;
    let total = provider_score + value_score + consistency_score;

    MergeDecision {
        value,
        score: total,
        reason: format!(
            "provider={provider_score:.3}, value_score={value_score:.3}, consistency={consistency_score:.3}"
        ),
    }
}

fn text_completeness(value: &str) -> f64 {
    let len = value.trim().chars().count() as f64;
    (len / 240.0).clamp(0.05, 1.0)
}

fn text_quality(field_name: &str, value: &str) -> f64 {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return 0.0;
    }

    let len = trimmed.chars().count() as f64;
    let html_penalty = if trimmed.contains('<') || trimmed.contains('>') { 0.15 } else { 0.0 };
    let newline_bonus = if trimmed.contains('\n') { 0.10 } else { 0.0 };
    let synopsis_bias = if field_name == "synopsis" {
        (len / 600.0).clamp(0.15, 1.0)
    } else {
        (len / 80.0).clamp(0.20, 1.0)
    };

    (synopsis_bias + newline_bonus - html_penalty).clamp(0.0, 1.0)
}

fn cover_quality(value: &str) -> f64 {
    let url = value.to_ascii_lowercase();
    let mut score: f64 = 0.35;

    if url.contains("large") || url.contains("extra") || url.contains("original") {
        score += 0.35;
    }
    if url.ends_with(".webp") {
        score += 0.15;
    }
    if url.starts_with("https://") {
        score += 0.15;
    }

    score.clamp(0.0, 1.0)
}

fn consistency_bonus(candidate: &CanonicalMedia) -> f64 {
    let mut filled: f64 = 0.0;

    if candidate.title_english.is_some() {
        filled += 0.20;
    }
    if candidate.title_romaji.is_some() {
        filled += 0.20;
    }
    if candidate.synopsis.is_some() {
        filled += 0.20;
    }
    if candidate.cover_image.is_some() {
        filled += 0.20;
    }
    if !candidate.external_ids.is_empty() {
        filled += 0.20;
    }

    filled.clamp(0.0, 1.0)
}
