//! Pure domain logic for merging canonical media and episode records.
//!
//! The merge engine is invoked on every upsert when an existing record is found
//! for the same external ID. It scores each incoming field against the stored
//! provenance and writes the higher-scoring value along with an audit trail
//! to [`field_provenance`](crate::model::CanonicalMedia::field_provenance).
//!
//! # Scoring formula
//!
//! Each field type uses a weighted sum of provider weight, completeness, and
//! quality signals. The exact weights differ per type (text, integer, boolean,
//! cover image) but all share the same provider priority order.
//!
//! # Provider priority (highest to lowest)
//!
//! AniList > IMDb > TVmaze > MyAnimeList > Jikan > Kitsu

use crate::model::{
    CanonicalEpisode, CanonicalMedia, EpisodeSourceRecord, ExternalId, FieldProvenance, SourceName,
    SourcePayload, StoredEpisode, StoredMedia,
};
use std::collections::HashMap;

/// Result of scoring a single field candidate from a provider.
#[derive(Debug, Clone)]
pub struct MergeDecision<T> {
    pub value: T,
    pub score: f64,
    pub reason: String,
}

/// Returns the 0.0–1.0 provider weight for merge scoring.
///
/// These weights reflect data completeness, normalization quality, and
/// the trustworthiness of IDs as stable identity anchors.
pub fn provider_weight(source: SourceName) -> f64 {
    match source {
        SourceName::AniList => 0.90,
        SourceName::Jikan => 0.76,
        SourceName::MyAnimeList => 0.80,
        SourceName::Kitsu => 0.78,
        SourceName::Tvmaze => 0.82,
        SourceName::Imdb => 0.85,
    }
}

/// Scores a text field candidate from a provider.
///
/// Combines provider weight, string completeness (length / 240), text quality
/// (HTML penalty, newline bonus, synopsis bias), and a consistency bonus
/// based on how many other fields on the candidate are already filled.
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
    let total =
        provider_score + completeness_score + field_quality_score + cover_score + consistency_score;

    MergeDecision {
        value: value.to_string(),
        score: total,
        reason: format!(
            "provider={provider_score:.3}, completeness={completeness_score:.3}, \
             quality={field_quality_score:.3}, consistency={consistency_score:.3}"
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
            "provider={provider_score:.3}, completeness={completeness_score:.3}, \
             numeric_quality={quality_score:.3}, consistency={consistency_score:.3}"
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
    let freshness_score = candidate
        .provider_rating
        .unwrap_or_default()
        .clamp(0.0, 1.0)
        * 0.10;
    let total = provider_score
        + completeness_score
        + cover_quality_score
        + consistency_score
        + freshness_score;

    MergeDecision {
        value: value.to_string(),
        score: total,
        reason: format!(
            "provider={provider_score:.3}, completeness={completeness_score:.3}, \
             cover_quality={cover_quality_score:.3}, consistency={consistency_score:.3}, \
             freshness={freshness_score:.3}"
        ),
    }
}

pub fn score_boolean(
    source: SourceName,
    value: bool,
    candidate: &CanonicalMedia,
) -> MergeDecision<bool> {
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
    let html_penalty = if trimmed.contains('<') || trimmed.contains('>') {
        0.15
    } else {
        0.0
    };
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

/// Merges an incoming [`CanonicalMedia`] with an optional existing [`StoredMedia`] record.
///
/// When `existing` is `None`, returns the incoming record unchanged (no merge needed).
/// When `existing` is `Some`, each field is independently scored and the higher-scoring
/// value is selected. Provenance is recorded for every field that changes.
///
/// The `media_kind` field is preserved from the existing record and never overwritten.
pub fn merge_media(existing: Option<&StoredMedia>, incoming: &CanonicalMedia) -> CanonicalMedia {
    let origin = incoming_origin(incoming);
    let existing_scores = existing_score_map(existing);
    let mut provenance = Vec::new();

    let title_display = choose_text(
        "title_display",
        existing.map(|item| item.title_display.as_str()),
        existing_scores.get("title_display"),
        Some(incoming.title_display.as_str()),
        incoming,
        &origin,
        &mut provenance,
    )
    .unwrap_or_else(|| incoming.title_display.clone());

    let title_romaji = choose_text(
        "title_romaji",
        existing.and_then(|item| item.title_romaji.as_deref()),
        existing_scores.get("title_romaji"),
        incoming.title_romaji.as_deref(),
        incoming,
        &origin,
        &mut provenance,
    );
    let title_english = choose_text(
        "title_english",
        existing.and_then(|item| item.title_english.as_deref()),
        existing_scores.get("title_english"),
        incoming.title_english.as_deref(),
        incoming,
        &origin,
        &mut provenance,
    );
    let title_native = choose_text(
        "title_native",
        existing.and_then(|item| item.title_native.as_deref()),
        existing_scores.get("title_native"),
        incoming.title_native.as_deref(),
        incoming,
        &origin,
        &mut provenance,
    );
    let synopsis = choose_text(
        "synopsis",
        existing.and_then(|item| item.synopsis.as_deref()),
        existing_scores.get("synopsis"),
        incoming.synopsis.as_deref(),
        incoming,
        &origin,
        &mut provenance,
    );
    let format = choose_text(
        "format",
        existing.and_then(|item| item.format.as_deref()),
        existing_scores.get("format"),
        incoming.format.as_deref(),
        incoming,
        &origin,
        &mut provenance,
    );
    let status = choose_text(
        "status",
        existing.and_then(|item| item.status.as_deref()),
        existing_scores.get("status"),
        incoming.status.as_deref(),
        incoming,
        &origin,
        &mut provenance,
    );
    let season = choose_text(
        "season",
        existing.and_then(|item| item.season.as_deref()),
        existing_scores.get("season"),
        incoming.season.as_deref(),
        incoming,
        &origin,
        &mut provenance,
    );
    let country_of_origin = choose_text(
        "country_of_origin",
        existing.and_then(|item| item.country_of_origin.as_deref()),
        existing_scores.get("country_of_origin"),
        incoming.country_of_origin.as_deref(),
        incoming,
        &origin,
        &mut provenance,
    );

    let season_year = choose_i32(
        "season_year",
        existing.and_then(|item| item.season_year),
        existing_scores.get("season_year"),
        incoming.season_year,
        incoming,
        &origin,
        &mut provenance,
    );
    let episodes = choose_i32(
        "episodes",
        existing.and_then(|item| item.episodes),
        existing_scores.get("episodes"),
        incoming.episodes,
        incoming,
        &origin,
        &mut provenance,
    );
    let chapters = choose_i32(
        "chapters",
        existing.and_then(|item| item.chapters),
        existing_scores.get("chapters"),
        incoming.chapters,
        incoming,
        &origin,
        &mut provenance,
    );
    let volumes = choose_i32(
        "volumes",
        existing.and_then(|item| item.volumes),
        existing_scores.get("volumes"),
        incoming.volumes,
        incoming,
        &origin,
        &mut provenance,
    );

    let cover_image = choose_cover(
        "cover_image",
        existing.and_then(|item| item.cover_image.as_deref()),
        existing_scores.get("cover_image"),
        incoming.cover_image.as_deref(),
        incoming,
        &origin,
        &mut provenance,
    );
    let banner_image = choose_cover(
        "banner_image",
        existing.and_then(|item| item.banner_image.as_deref()),
        existing_scores.get("banner_image"),
        incoming.banner_image.as_deref(),
        incoming,
        &origin,
        &mut provenance,
    );
    let nsfw = choose_bool(
        "nsfw",
        existing.map(|item| item.nsfw),
        existing_scores.get("nsfw"),
        incoming.nsfw,
        incoming,
        &origin,
        &mut provenance,
    );
    let provider_rating = choose_rating(
        existing.and_then(|item| item.provider_rating),
        incoming.provider_rating,
    );

    CanonicalMedia {
        media_kind: existing
            .map(|item| item.media_kind)
            .unwrap_or(incoming.media_kind),
        title_display,
        title_romaji,
        title_english,
        title_native,
        synopsis,
        format,
        status,
        season,
        season_year,
        episodes,
        chapters,
        volumes,
        country_of_origin,
        cover_image,
        banner_image,
        provider_rating,
        nsfw,
        aliases: merge_string_lists(
            existing.map(|item| item.aliases.as_slice()),
            &incoming.aliases,
        ),
        genres: merge_string_lists(
            existing.map(|item| item.genres.as_slice()),
            &incoming.genres,
        ),
        tags: merge_string_lists(existing.map(|item| item.tags.as_slice()), &incoming.tags),
        external_ids: merge_external_ids(
            existing.map(|item| item.external_ids.as_slice()),
            &incoming.external_ids,
        ),
        source_payloads: merge_source_payloads(
            existing.map(|item| item.source_payloads.as_slice()),
            &incoming.source_payloads,
        ),
        field_provenance: provenance,
    }
}

/// Merges multiple source episode records for the same media+episode into one canonical record.
///
/// Groups records by `(media_id, absolute_number)` or `(media_id, season_number, episode_number)`,
/// then selects field values from the highest-priority provider per field using
/// the episode provider priority order.
///
/// Priority order (highest to lowest): AniList > IMDb > TVmaze > MyAnimeList > Jikan > Kitsu
pub fn merge_episode_source_records(records: &[EpisodeSourceRecord]) -> StoredEpisode {
    // Sort by priority descending
    let mut sorted = records.to_vec();
    sorted.sort_by_key(|r| episode_provider_priority(r.source));
    let highest = &sorted[0];

    fn pick<T: Clone>(values: &[(Option<T>, SourceName)]) -> Option<T> {
        // Sort by priority descending and take the last (highest priority)
        let mut with_prio: Vec<_> = values
            .iter()
            .filter_map(|(v, s)| v.as_ref().map(|val| (val, *s)))
            .collect();
        with_prio.sort_by_key(|(_, s)| episode_provider_priority(*s));
        with_prio.last().map(|(v, _)| (*v).clone())
    }

    let title_display = pick(
        &sorted
            .iter()
            .map(|r| (r.title_display.clone(), r.source))
            .collect::<Vec<_>>(),
    );
    let title_original = pick(
        &sorted
            .iter()
            .map(|r| (r.title_original.clone(), r.source))
            .collect::<Vec<_>>(),
    );
    let synopsis = pick(
        &sorted
            .iter()
            .map(|r| (r.synopsis.clone(), r.source))
            .collect::<Vec<_>>(),
    );
    let air_date = pick(
        &sorted
            .iter()
            .map(|r| (r.air_date.clone(), r.source))
            .collect::<Vec<_>>(),
    );
    let runtime_minutes = pick(
        &sorted
            .iter()
            .map(|r| (r.runtime_minutes, r.source))
            .collect::<Vec<_>>(),
    );
    let thumbnail_url = pick(
        &sorted
            .iter()
            .map(|r| (r.thumbnail_url.clone(), r.source))
            .collect::<Vec<_>>(),
    );
    let titles_json = highest.titles_json.clone();

    StoredEpisode {
        id: 0, // Will be assigned on insert
        media_id: highest.media_id,
        season_number: highest.season_number,
        episode_number: highest.episode_number,
        absolute_number: highest.absolute_number,
        title_display,
        title_original,
        titles_json,
        synopsis,
        air_date,
        runtime_minutes,
        thumbnail_url,
    }
}

/// Merges raw remote episode records into one record per flat effective episode number.
///
/// This is a persistence-free companion to [`merge_episode_source_records`] for callers that
/// use [`RemoteApi::fetch_episodes_from_external_ids`](crate::RemoteApi::fetch_episodes_from_external_ids)
/// without the local SQLite repository. Records are grouped by
/// `absolute_number.or(episode_number)`, so absolute/global numbering wins when present and
/// season numbers are intentionally ignored. Records with neither number are skipped.
///
/// Field values are selected independently from the highest-priority provider that supplied a
/// non-null value. For anime episode aggregation this means Jikan fills or overrides Kitsu per
/// field, while Kitsu can still fill fields missing from Jikan.
pub fn merge_canonical_episodes_by_effective_number(
    episodes: &[CanonicalEpisode],
) -> Vec<CanonicalEpisode> {
    let mut groups: HashMap<i32, Vec<&CanonicalEpisode>> = HashMap::new();

    for episode in episodes {
        if let Some(effective_number) = episode.absolute_number.or(episode.episode_number) {
            groups.entry(effective_number).or_default().push(episode);
        }
    }

    let mut merged: Vec<_> = groups
        .into_values()
        .map(|group| merge_canonical_episode_group(&group))
        .collect();
    merged.sort_by_key(|episode| episode.absolute_number.or(episode.episode_number));
    merged
}

fn merge_canonical_episode_group(group: &[&CanonicalEpisode]) -> CanonicalEpisode {
    let identity = group
        .iter()
        .max_by_key(|episode| episode_provider_priority(episode.source))
        .expect("episode merge group must not be empty");

    CanonicalEpisode {
        source: identity.source,
        source_id: identity.source_id.clone(),
        media_kind: identity.media_kind,
        season_number: identity.season_number,
        episode_number: identity.episode_number,
        absolute_number: identity.absolute_number,
        title_display: pick_episode_field(group, |episode| episode.title_display.clone()),
        title_original: pick_episode_field(group, |episode| episode.title_original.clone()),
        synopsis: pick_episode_field(group, |episode| episode.synopsis.clone()),
        air_date: pick_episode_field(group, |episode| episode.air_date.clone()),
        runtime_minutes: pick_episode_field(group, |episode| episode.runtime_minutes),
        thumbnail_url: pick_episode_field(group, |episode| episode.thumbnail_url.clone()),
        raw_titles_json: pick_episode_field(group, |episode| episode.raw_titles_json.clone()),
        raw_json: identity.raw_json.clone(),
    }
}

fn pick_episode_field<T: Clone>(
    group: &[&CanonicalEpisode],
    field: impl Fn(&CanonicalEpisode) -> Option<T>,
) -> Option<T> {
    group
        .iter()
        .filter_map(|episode| {
            field(episode).map(|value| (episode_provider_priority(episode.source), value))
        })
        .max_by_key(|(priority, _)| *priority)
        .map(|(_, value)| value)
}

fn episode_provider_priority(source: SourceName) -> u8 {
    match source {
        SourceName::AniList => 5,
        SourceName::MyAnimeList => 4,
        SourceName::Jikan => 3,
        SourceName::Kitsu => 2,
        SourceName::Tvmaze => 4,
        SourceName::Imdb => 5,
    }
}

fn choose_text(
    field_name: &str,
    existing_value: Option<&str>,
    existing_provenance: Option<&FieldProvenance>,
    incoming_value: Option<&str>,
    incoming: &CanonicalMedia,
    origin: &(SourceName, String),
    provenance: &mut Vec<FieldProvenance>,
) -> Option<String> {
    match (existing_value, incoming_value) {
        (Some(existing), Some(candidate)) => {
            let existing_score = existing_provenance.map(|item| item.score).unwrap_or(0.60);
            let incoming_decision = score_text_field(origin.0, field_name, candidate, incoming);
            if incoming_decision.score >= existing_score {
                provenance.push(make_provenance(
                    field_name,
                    origin.0,
                    origin.1.as_str(),
                    incoming_decision.score,
                    incoming_decision.reason,
                ));
                Some(incoming_decision.value)
            } else {
                if let Some(entry) = existing_provenance.cloned() {
                    provenance.push(entry);
                }
                Some(existing.to_string())
            }
        }
        (None, Some(candidate)) => {
            let incoming_decision = score_text_field(origin.0, field_name, candidate, incoming);
            provenance.push(make_provenance(
                field_name,
                origin.0,
                origin.1.as_str(),
                incoming_decision.score,
                incoming_decision.reason,
            ));
            Some(incoming_decision.value)
        }
        (Some(existing), None) => {
            if let Some(entry) = existing_provenance.cloned() {
                provenance.push(entry);
            }
            Some(existing.to_string())
        }
        (None, None) => None,
    }
}

fn choose_i32(
    field_name: &str,
    existing_value: Option<i32>,
    existing_provenance: Option<&FieldProvenance>,
    incoming_value: Option<i32>,
    incoming: &CanonicalMedia,
    origin: &(SourceName, String),
    provenance: &mut Vec<FieldProvenance>,
) -> Option<i32> {
    match (existing_value, incoming_value) {
        (Some(existing), Some(candidate)) => {
            let existing_score = existing_provenance.map(|item| item.score).unwrap_or(0.60);
            let incoming_decision = score_optional_i32(origin.0, candidate, incoming);
            if incoming_decision.score >= existing_score {
                provenance.push(make_provenance(
                    field_name,
                    origin.0,
                    origin.1.as_str(),
                    incoming_decision.score,
                    incoming_decision.reason,
                ));
                Some(incoming_decision.value)
            } else {
                if let Some(entry) = existing_provenance.cloned() {
                    provenance.push(entry);
                }
                Some(existing)
            }
        }
        (None, Some(candidate)) => {
            let incoming_decision = score_optional_i32(origin.0, candidate, incoming);
            provenance.push(make_provenance(
                field_name,
                origin.0,
                origin.1.as_str(),
                incoming_decision.score,
                incoming_decision.reason,
            ));
            Some(incoming_decision.value)
        }
        (Some(existing), None) => {
            if let Some(entry) = existing_provenance.cloned() {
                provenance.push(entry);
            }
            Some(existing)
        }
        (None, None) => None,
    }
}

fn choose_cover(
    field_name: &str,
    existing_value: Option<&str>,
    existing_provenance: Option<&FieldProvenance>,
    incoming_value: Option<&str>,
    incoming: &CanonicalMedia,
    origin: &(SourceName, String),
    provenance: &mut Vec<FieldProvenance>,
) -> Option<String> {
    match (existing_value, incoming_value) {
        (Some(existing), Some(candidate)) => {
            let existing_score = existing_provenance.map(|item| item.score).unwrap_or(0.60);
            let incoming_decision = score_cover_image(origin.0, candidate, incoming);
            if incoming_decision.score >= existing_score {
                provenance.push(make_provenance(
                    field_name,
                    origin.0,
                    origin.1.as_str(),
                    incoming_decision.score,
                    incoming_decision.reason,
                ));
                Some(incoming_decision.value)
            } else {
                if let Some(entry) = existing_provenance.cloned() {
                    provenance.push(entry);
                }
                Some(existing.to_string())
            }
        }
        (None, Some(candidate)) => {
            let incoming_decision = score_cover_image(origin.0, candidate, incoming);
            provenance.push(make_provenance(
                field_name,
                origin.0,
                origin.1.as_str(),
                incoming_decision.score,
                incoming_decision.reason,
            ));
            Some(incoming_decision.value)
        }
        (Some(existing), None) => {
            if let Some(entry) = existing_provenance.cloned() {
                provenance.push(entry);
            }
            Some(existing.to_string())
        }
        (None, None) => None,
    }
}

fn choose_bool(
    field_name: &str,
    existing_value: Option<bool>,
    existing_provenance: Option<&FieldProvenance>,
    incoming_value: bool,
    incoming: &CanonicalMedia,
    origin: &(SourceName, String),
    provenance: &mut Vec<FieldProvenance>,
) -> bool {
    match existing_value {
        Some(existing) => {
            let existing_score = existing_provenance.map(|item| item.score).unwrap_or(0.60);
            let incoming_decision = score_boolean(origin.0, incoming_value, incoming);
            if incoming_decision.score >= existing_score {
                provenance.push(make_provenance(
                    field_name,
                    origin.0,
                    origin.1.as_str(),
                    incoming_decision.score,
                    incoming_decision.reason,
                ));
                incoming_decision.value
            } else {
                if let Some(entry) = existing_provenance.cloned() {
                    provenance.push(entry);
                }
                existing
            }
        }
        None => {
            let incoming_decision = score_boolean(origin.0, incoming_value, incoming);
            provenance.push(make_provenance(
                field_name,
                origin.0,
                origin.1.as_str(),
                incoming_decision.score,
                incoming_decision.reason,
            ));
            incoming_decision.value
        }
    }
}

fn merge_external_ids(existing: Option<&[ExternalId]>, incoming: &[ExternalId]) -> Vec<ExternalId> {
    let mut values = Vec::new();
    for item in existing.into_iter().flatten() {
        if !values.iter().any(|value: &ExternalId| {
            value.source == item.source && value.source_id == item.source_id
        }) {
            values.push(item.clone());
        }
    }
    for item in incoming {
        if !values.iter().any(|value: &ExternalId| {
            value.source == item.source && value.source_id == item.source_id
        }) {
            values.push(item.clone());
        }
    }
    values
}

fn merge_source_payloads(
    existing: Option<&[SourcePayload]>,
    incoming: &[SourcePayload],
) -> Vec<SourcePayload> {
    let mut values = Vec::new();
    for item in existing.into_iter().flatten() {
        if !values.iter().any(|value: &SourcePayload| {
            value.source == item.source && value.source_id == item.source_id
        }) {
            values.push(item.clone());
        }
    }
    for item in incoming {
        if let Some(existing_item) = values.iter_mut().find(|value: &&mut SourcePayload| {
            value.source == item.source && value.source_id == item.source_id
        }) {
            *existing_item = item.clone();
        } else {
            values.push(item.clone());
        }
    }
    values
}

pub fn make_provenance(
    field_name: &str,
    source: SourceName,
    source_id: &str,
    score: f64,
    reason: String,
) -> FieldProvenance {
    FieldProvenance {
        field_name: field_name.to_string(),
        source,
        source_id: source_id.to_string(),
        score,
        reason,
        updated_at: String::new(), // Will be populated by DB DEFAULT
    }
}

fn incoming_origin(media: &CanonicalMedia) -> (SourceName, String) {
    if let Some(payload) = media.source_payloads.first() {
        return (payload.source, payload.source_id.clone());
    }
    if let Some(external_id) = media.external_ids.first() {
        return (external_id.source, external_id.source_id.clone());
    }
    (SourceName::AniList, "unknown".to_string())
}

fn existing_score_map(existing: Option<&StoredMedia>) -> HashMap<String, FieldProvenance> {
    existing
        .map(|item| {
            item.field_provenance
                .iter()
                .cloned()
                .map(|entry| (entry.field_name.clone(), entry))
                .collect()
        })
        .unwrap_or_default()
}

fn choose_rating(existing_value: Option<f64>, incoming_value: Option<f64>) -> Option<f64> {
    match (existing_value, incoming_value) {
        (Some(existing), Some(candidate)) => Some(existing.max(candidate)),
        (None, Some(candidate)) => Some(candidate),
        (Some(existing), None) => Some(existing),
        (None, None) => None,
    }
}

fn merge_string_lists(existing: Option<&[String]>, incoming: &[String]) -> Vec<String> {
    let mut values = Vec::new();
    for value in existing.into_iter().flatten() {
        if !values
            .iter()
            .any(|item: &String| item.eq_ignore_ascii_case(value))
        {
            values.push(value.clone());
        }
    }
    for value in incoming {
        if !values
            .iter()
            .any(|item: &String| item.eq_ignore_ascii_case(value))
        {
            values.push(value.clone());
        }
    }
    values
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::MediaKind;

    fn episode(
        source: SourceName,
        source_id: &str,
        absolute_number: Option<i32>,
        episode_number: Option<i32>,
    ) -> CanonicalEpisode {
        CanonicalEpisode {
            source,
            source_id: source_id.to_string(),
            media_kind: MediaKind::Anime,
            season_number: None,
            episode_number,
            absolute_number,
            title_display: None,
            title_original: None,
            synopsis: None,
            air_date: None,
            runtime_minutes: None,
            thumbnail_url: None,
            raw_titles_json: None,
            raw_json: None,
        }
    }

    #[test]
    fn merges_canonical_episodes_by_field_priority() {
        let mut kitsu = episode(SourceName::Kitsu, "kitsu-5", None, Some(5));
        kitsu.title_display = Some("Kitsu title".to_string());
        kitsu.synopsis = Some("Kitsu synopsis".to_string());
        kitsu.runtime_minutes = Some(24);
        kitsu.air_date = Some("2024-01-01".to_string());

        let mut jikan = episode(SourceName::Jikan, "mal-5", Some(5), Some(5));
        jikan.title_display = Some("Jikan title".to_string());
        jikan.air_date = Some("2024-01-02".to_string());

        let merged = merge_canonical_episodes_by_effective_number(&[kitsu, jikan]);

        assert_eq!(merged.len(), 1);
        let episode = &merged[0];
        assert_eq!(episode.source, SourceName::Jikan);
        assert_eq!(episode.source_id, "mal-5");
        assert_eq!(episode.absolute_number, Some(5));
        assert_eq!(episode.title_display.as_deref(), Some("Jikan title"));
        assert_eq!(episode.synopsis.as_deref(), Some("Kitsu synopsis"));
        assert_eq!(episode.runtime_minutes, Some(24));
        assert_eq!(episode.air_date.as_deref(), Some("2024-01-02"));
    }

    #[test]
    fn skips_canonical_episodes_without_effective_number() {
        let invalid = episode(SourceName::Jikan, "missing-number", None, None);
        let valid = episode(SourceName::Kitsu, "kitsu-2", None, Some(2));

        let merged = merge_canonical_episodes_by_effective_number(&[invalid, valid]);

        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].episode_number, Some(2));
    }

    #[test]
    fn groups_by_absolute_number_before_episode_number() {
        let season_relative = episode(SourceName::Kitsu, "kitsu-s2e1", Some(13), Some(1));
        let absolute = episode(SourceName::Jikan, "mal-13", Some(13), Some(13));

        let merged = merge_canonical_episodes_by_effective_number(&[season_relative, absolute]);

        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].absolute_number, Some(13));
        assert_eq!(merged[0].source, SourceName::Jikan);
    }
}
