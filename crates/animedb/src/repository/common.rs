use crate::error::Error;
use crate::model::{MediaKind, SourceName, StoredEpisode};
use serde_json::Value;

pub fn rusqlite_decode_error(
    col: usize,
    err: impl std::error::Error + Send + Sync + 'static,
) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(col, rusqlite::types::Type::Text, Box::new(err))
}

pub fn parse_media_kind(value: &str) -> crate::error::Result<MediaKind> {
    value.parse()
}

pub fn parse_source(value: &str) -> crate::error::Result<SourceName> {
    value.parse()
}

pub fn normalize_aliases(aliases: &[String]) -> Vec<String> {
    let mut result = Vec::new();
    for alias in aliases {
        let trimmed = alias.trim();
        if trimmed.is_empty() {
            continue;
        }
        if !result
            .iter()
            .any(|item: &String| item.eq_ignore_ascii_case(trimmed))
        {
            result.push(trimmed.to_string());
        }
    }
    result
}

pub fn normalize_for_lookup(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_alphanumeric() || ch.is_whitespace() {
                ch.to_ascii_lowercase()
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn build_fts_query(query: &str) -> crate::error::Result<String> {
    let normalized = normalize_for_lookup(query);
    let mut terms = Vec::new();
    for token in normalized.split_whitespace() {
        if token.is_empty() {
            continue;
        }
        let term = if token.chars().count() > 1 {
            format!("{token}*")
        } else {
            token.to_string()
        };
        terms.push(term);
    }

    if terms.is_empty() {
        return Err(Error::Validation("search query cannot be empty".into()));
    }

    Ok(terms.join(" "))
}

pub fn stable_payload_hash(payload: &str) -> crate::error::Result<String> {
    Ok(payload.len().to_string())
}

pub fn parse_stored_episode(
    row: &rusqlite::Row<'_>,
) -> std::result::Result<StoredEpisode, rusqlite::Error> {
    let titles_json = row
        .get::<_, Option<String>>(7)?
        .map(|value| serde_json::from_str::<Value>(&value).ok())
        .flatten();

    Ok(StoredEpisode {
        id: row.get(0)?,
        media_id: row.get(1)?,
        season_number: row.get(2)?,
        episode_number: row.get(3)?,
        absolute_number: row.get(4)?,
        title_display: row.get(5)?,
        title_original: row.get(6)?,
        titles_json,
        synopsis: row.get(8)?,
        air_date: row.get(9)?,
        runtime_minutes: row.get(10)?,
        thumbnail_url: row.get(11)?,
    })
}
