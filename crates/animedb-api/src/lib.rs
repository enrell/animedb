//! GraphQL service layer for the `animedb` catalog crate.
//!
//! This crate exposes:
//!
//! - a reusable GraphQL schema builder via [`build_schema`]
//! - an Axum router via [`build_router`]
//! - a binary entry point in `main.rs` for running the service directly

use animedb::{
    AniListProvider, AnimeDb, CanonicalMedia, FieldProvenance, ImdbProvider, JikanProvider,
    KitsuProvider, MediaKind, PersistedSyncState, RemoteCatalog, SearchHit, SearchOptions,
    SourceName, StoredMedia, SyncMode, SyncReport, SyncRequest, TvmazeProvider,
};
use async_graphql::http::{GraphQLPlaygroundConfig, playground_source};
use async_graphql::{
    Context, EmptySubscription, Enum, InputObject, Object, Result as GraphQLResult, Schema,
    SimpleObject,
};
use async_graphql_axum::{GraphQLRequest, GraphQLResponse};
use axum::extract::State;
use axum::response::{Html, IntoResponse};
use axum::routing::{get, post};
use axum::{Json, Router};
use std::path::PathBuf;

pub type AnimeDbSchema = Schema<QueryRoot, MutationRoot, EmptySubscription>;

/// Shared application state injected into the GraphQL schema.
#[derive(Clone)]
pub struct AppState {
    pub database_path: PathBuf,
}

/// Builds the GraphQL schema backed by one SQLite database path.
pub fn build_schema(database_path: impl Into<PathBuf>) -> AnimeDbSchema {
    Schema::build(QueryRoot, MutationRoot, EmptySubscription)
        .data(AppState {
            database_path: database_path.into(),
        })
        .finish()
}

/// Builds the Axum router that serves Playground, GraphQL, and health endpoints.
pub fn build_router(schema: AnimeDbSchema) -> Router {
    Router::new()
        .route("/", get(playground).post(graphql_handler))
        .route("/graphql", post(graphql_handler))
        .route("/healthz", get(healthz))
        .with_state(schema)
}

pub struct QueryRoot;

#[Object]
impl QueryRoot {
    async fn health(&self) -> &'static str {
        "ok"
    }

    async fn media(&self, ctx: &Context<'_>, id: i64) -> GraphQLResult<Option<MediaObject>> {
        let state = ctx.data_unchecked::<AppState>().clone();
        let media = tokio::task::spawn_blocking(move || {
            let db = AnimeDb::open(&state.database_path)?;
            db.media().get_media(id).map(MediaObject::from).map(Some)
        })
        .await
        .map_err(join_error)?
        .or_else(not_found_is_none)?;

        Ok(media)
    }

    async fn media_by_external_id(
        &self,
        ctx: &Context<'_>,
        source: SourceNameObject,
        source_id: String,
    ) -> GraphQLResult<Option<MediaObject>> {
        let state = ctx.data_unchecked::<AppState>().clone();
        let source = source.into_model();
        let media = tokio::task::spawn_blocking(move || {
            let db = AnimeDb::open(&state.database_path)?;
            db.media()
                .get_by_external_id(source, &source_id)
                .map(MediaObject::from)
                .map(Some)
        })
        .await
        .map_err(join_error)?
        .or_else(not_found_is_none)?;

        Ok(media)
    }

    async fn search(
        &self,
        ctx: &Context<'_>,
        query: String,
        options: Option<SearchInput>,
    ) -> GraphQLResult<Vec<SearchHitObject>> {
        let state = ctx.data_unchecked::<AppState>().clone();
        let options = options.unwrap_or_default().into_model();
        let hits = tokio::task::spawn_blocking(move || {
            let db = AnimeDb::open(&state.database_path)?;
            db.search_repo()
                .search(&query, options)
                .map(|items| items.into_iter().map(SearchHitObject::from).collect())
        })
        .await
        .map_err(join_error)??;

        Ok(hits)
    }

    async fn sync_state(
        &self,
        ctx: &Context<'_>,
        source: SourceNameObject,
        scope: String,
    ) -> GraphQLResult<Option<SyncStateObject>> {
        let state = ctx.data_unchecked::<AppState>().clone();
        let source = source.into_model();
        let sync_state = tokio::task::spawn_blocking(move || {
            let db = AnimeDb::open(&state.database_path)?;
            db.sync_state()
                .load_sync_state(source, &scope)
                .map(SyncStateObject::from)
                .map(Some)
        })
        .await
        .map_err(join_error)?
        .or_else(not_found_is_none)?;

        Ok(sync_state)
    }

    async fn remote_search(
        &self,
        source: SourceNameObject,
        query: String,
        options: Option<SearchInput>,
    ) -> GraphQLResult<Vec<MediaObject>> {
        let options = options.unwrap_or_default().into_model();
        let media = tokio::task::spawn_blocking(move || search_remote(source, &query, options))
            .await
            .map_err(join_error)??;

        Ok(media)
    }

    async fn remote_media(
        &self,
        source: SourceNameObject,
        source_id: String,
        media_kind: MediaKindObject,
    ) -> GraphQLResult<Option<MediaObject>> {
        let media =
            tokio::task::spawn_blocking(move || get_remote_media(source, &source_id, media_kind))
                .await
                .map_err(join_error)??;

        Ok(media)
    }
}

pub struct MutationRoot;

#[Object]
impl MutationRoot {
    async fn generate_database(
        &self,
        ctx: &Context<'_>,
        max_pages: Option<i32>,
    ) -> GraphQLResult<SyncReportObject> {
        let state = ctx.data_unchecked::<AppState>().clone();
        let max_pages = max_pages.map(|value| value.max(1) as usize);

        let report = tokio::task::spawn_blocking(move || {
            let mut db = AnimeDb::open(&state.database_path)?;
            if let Some(max_pages) = max_pages {
                sync_with_max_pages(&mut db, max_pages)
            } else {
                db.sync_default_sources()
            }
        })
        .await
        .map_err(join_error)??;

        Ok(report.into())
    }

    async fn sync_database(
        &self,
        ctx: &Context<'_>,
        input: Option<SyncInput>,
    ) -> GraphQLResult<SyncReportObject> {
        let state = ctx.data_unchecked::<AppState>().clone();
        let input = input.unwrap_or_default();

        let report = tokio::task::spawn_blocking(move || {
            let mut db = AnimeDb::open(&state.database_path)?;
            if let Some(source) = input.source {
                let mut request = SyncRequest::new(source.into_model());
                if let Some(kind) = input.media_kind {
                    request = request.with_media_kind(kind.into_model());
                }
                if let Some(page_size) = input.page_size {
                    request = request.with_page_size(page_size.max(1) as usize);
                }
                if let Some(max_pages) = input.max_pages {
                    request = request.with_max_pages(max_pages.max(1) as usize);
                }

                let outcome = match source {
                    SourceNameObject::Anilist => {
                        db.sync_from(&AniListProvider::default(), request)?
                    }
                    SourceNameObject::Jikan => db.sync_from(&JikanProvider::default(), request)?,
                    SourceNameObject::Kitsu => db.sync_from(&KitsuProvider::default(), request)?,
                    SourceNameObject::Tvmaze => {
                        db.sync_from(&TvmazeProvider::default(), request)?
                    }
                    SourceNameObject::Imdb => db.sync_from(&ImdbProvider::default(), request)?,
                    SourceNameObject::Myanimelist => {
                        return Err(animedb::Error::Validation(
                            "sync direto para MyAnimeList não existe; use AniList, Jikan, Kitsu, Tvmaze ou Imdb"
                                .into(),
                        ));
                    }
                };
                Ok(SyncReport {
                    total_upserted_records: outcome.upserted_records,
                    outcomes: vec![outcome],
                })
            } else if let Some(max_pages) = input.max_pages {
                sync_with_max_pages(&mut db, max_pages.max(1) as usize)
            } else {
                db.sync_default_sources()
            }
        })
        .await
        .map_err(join_error)??;

        Ok(report.into())
    }
}

#[derive(Default, InputObject)]
struct SearchInput {
    limit: Option<i32>,
    offset: Option<i32>,
    media_kind: Option<MediaKindObject>,
    format: Option<String>,
}

impl SearchInput {
    fn into_model(self) -> SearchOptions {
        let mut options = SearchOptions::default();
        if let Some(limit) = self.limit {
            options = options.with_limit(limit.max(1) as usize);
        }
        if let Some(offset) = self.offset {
            options = options.with_offset(offset.max(0) as usize);
        }
        if let Some(media_kind) = self.media_kind {
            options = options.with_media_kind(media_kind.into_model());
        }
        if let Some(format) = self.format {
            options = options.with_format(format);
        }
        options
    }
}

#[derive(Default, InputObject)]
struct SyncInput {
    source: Option<SourceNameObject>,
    media_kind: Option<MediaKindObject>,
    max_pages: Option<i32>,
    page_size: Option<i32>,
}

#[derive(Copy, Clone, Eq, PartialEq, Enum)]
enum MediaKindObject {
    Anime,
    Manga,
    Show,
    Movie,
}

impl MediaKindObject {
    fn into_model(self) -> MediaKind {
        match self {
            Self::Anime => MediaKind::Anime,
            Self::Manga => MediaKind::Manga,
            Self::Show => MediaKind::Show,
            Self::Movie => MediaKind::Movie,
        }
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Enum)]
enum SourceNameObject {
    Anilist,
    Myanimelist,
    Jikan,
    Kitsu,
    Tvmaze,
    Imdb,
}

impl SourceNameObject {
    fn into_model(self) -> SourceName {
        match self {
            Self::Anilist => SourceName::AniList,
            Self::Myanimelist => SourceName::MyAnimeList,
            Self::Jikan => SourceName::Jikan,
            Self::Kitsu => SourceName::Kitsu,
            Self::Tvmaze => SourceName::Tvmaze,
            Self::Imdb => SourceName::Imdb,
        }
    }
}

#[derive(SimpleObject)]
struct ExternalIdObject {
    source: SourceNameObject,
    source_id: String,
    url: Option<String>,
}

#[derive(SimpleObject)]
struct SourcePayloadObject {
    source: SourceNameObject,
    source_id: String,
    url: Option<String>,
    remote_updated_at: Option<String>,
    raw_json: Option<String>,
}

#[derive(SimpleObject)]
struct MediaObject {
    id: i64,
    media_kind: MediaKindObject,
    name: String,
    title_display: String,
    title_romaji: Option<String>,
    title_english: Option<String>,
    title_native: Option<String>,
    synopsis: Option<String>,
    format: Option<String>,
    status: Option<String>,
    season: Option<String>,
    season_year: Option<i32>,
    episodes: Option<i32>,
    chapters: Option<i32>,
    volumes: Option<i32>,
    country_of_origin: Option<String>,
    cover_image: Option<String>,
    banner_image: Option<String>,
    provider_rating: Option<f64>,
    nsfw: bool,
    aliases: Vec<String>,
    genres: Vec<String>,
    tags: Vec<String>,
    external_ids: Vec<ExternalIdObject>,
    source_payloads: Vec<SourcePayloadObject>,
    field_provenance: Vec<FieldProvenanceObject>,
}

impl MediaObject {
    fn from_canonical(media: CanonicalMedia) -> Self {
        Self {
            id: -1,
            media_kind: media.media_kind.into(),
            name: media.title_display.clone(),
            title_display: media.title_display,
            title_romaji: media.title_romaji,
            title_english: media.title_english,
            title_native: media.title_native,
            synopsis: media.synopsis,
            format: media.format,
            status: media.status,
            season: media.season,
            season_year: media.season_year,
            episodes: media.episodes,
            chapters: media.chapters,
            volumes: media.volumes,
            country_of_origin: media.country_of_origin,
            cover_image: media.cover_image,
            banner_image: media.banner_image,
            provider_rating: media.provider_rating,
            nsfw: media.nsfw,
            aliases: media.aliases,
            genres: media.genres,
            tags: media.tags,
            external_ids: media
                .external_ids
                .into_iter()
                .map(ExternalIdObject::from)
                .collect(),
            source_payloads: media
                .source_payloads
                .into_iter()
                .map(SourcePayloadObject::from)
                .collect(),
            field_provenance: media
                .field_provenance
                .into_iter()
                .map(FieldProvenanceObject::from)
                .collect(),
        }
    }
}

impl From<StoredMedia> for MediaObject {
    fn from(media: StoredMedia) -> Self {
        Self {
            id: media.id,
            media_kind: media.media_kind.into(),
            name: media.title_display.clone(),
            title_display: media.title_display,
            title_romaji: media.title_romaji,
            title_english: media.title_english,
            title_native: media.title_native,
            synopsis: media.synopsis,
            format: media.format,
            status: media.status,
            season: media.season,
            season_year: media.season_year,
            episodes: media.episodes,
            chapters: media.chapters,
            volumes: media.volumes,
            country_of_origin: media.country_of_origin,
            cover_image: media.cover_image,
            banner_image: media.banner_image,
            provider_rating: media.provider_rating,
            nsfw: media.nsfw,
            aliases: media.aliases,
            genres: media.genres,
            tags: media.tags,
            external_ids: media
                .external_ids
                .into_iter()
                .map(ExternalIdObject::from)
                .collect(),
            source_payloads: media
                .source_payloads
                .into_iter()
                .map(SourcePayloadObject::from)
                .collect(),
            field_provenance: media
                .field_provenance
                .into_iter()
                .map(FieldProvenanceObject::from)
                .collect(),
        }
    }
}

#[derive(SimpleObject)]
struct FieldProvenanceObject {
    field_name: String,
    source: SourceNameObject,
    source_id: String,
    score: f64,
    reason: String,
    updated_at: String,
}

impl From<FieldProvenance> for FieldProvenanceObject {
    fn from(value: FieldProvenance) -> Self {
        Self {
            field_name: value.field_name,
            source: value.source.into(),
            source_id: value.source_id,
            score: value.score,
            reason: value.reason,
            updated_at: value.updated_at,
        }
    }
}

#[derive(SimpleObject)]
struct SearchHitObject {
    media_id: i64,
    media_kind: MediaKindObject,
    name: String,
    title_display: String,
    synopsis: Option<String>,
    score: f64,
}

impl From<SearchHit> for SearchHitObject {
    fn from(hit: SearchHit) -> Self {
        Self {
            media_id: hit.media_id,
            media_kind: hit.media_kind.into(),
            name: hit.title_display.clone(),
            title_display: hit.title_display,
            synopsis: hit.synopsis,
            score: hit.score,
        }
    }
}

#[derive(SimpleObject)]
struct SyncOutcomeObject {
    source: SourceNameObject,
    media_kind: Option<MediaKindObject>,
    fetched_pages: usize,
    upserted_records: usize,
    last_cursor_page: Option<usize>,
}

#[derive(SimpleObject)]
struct SyncReportObject {
    total_upserted_records: usize,
    outcomes: Vec<SyncOutcomeObject>,
}

impl From<SyncReport> for SyncReportObject {
    fn from(report: SyncReport) -> Self {
        Self {
            total_upserted_records: report.total_upserted_records,
            outcomes: report
                .outcomes
                .into_iter()
                .map(SyncOutcomeObject::from)
                .collect(),
        }
    }
}

impl From<animedb::SyncOutcome> for SyncOutcomeObject {
    fn from(outcome: animedb::SyncOutcome) -> Self {
        Self {
            source: outcome.source.into(),
            media_kind: outcome.media_kind.map(Into::into),
            fetched_pages: outcome.fetched_pages,
            upserted_records: outcome.upserted_records,
            last_cursor_page: outcome.last_cursor.map(|cursor| cursor.page),
        }
    }
}

#[derive(SimpleObject)]
struct SyncStateObject {
    source: SourceNameObject,
    scope: String,
    cursor_page: Option<usize>,
    last_success_at: Option<String>,
    last_error: Option<String>,
    last_page: Option<i64>,
    mode: SyncModeObject,
}

impl From<PersistedSyncState> for SyncStateObject {
    fn from(state: PersistedSyncState) -> Self {
        Self {
            source: state.source.into(),
            scope: state.scope,
            cursor_page: state.cursor.map(|cursor| cursor.page),
            last_success_at: state.last_success_at,
            last_error: state.last_error,
            last_page: state.last_page,
            mode: state.mode.into(),
        }
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Enum)]
enum SyncModeObject {
    Full,
    Incremental,
}

impl From<SyncMode> for SyncModeObject {
    fn from(mode: SyncMode) -> Self {
        match mode {
            SyncMode::Full => Self::Full,
            SyncMode::Incremental => Self::Incremental,
        }
    }
}

impl From<MediaKind> for MediaKindObject {
    fn from(value: MediaKind) -> Self {
        match value {
            MediaKind::Anime => Self::Anime,
            MediaKind::Manga => Self::Manga,
            MediaKind::Show => Self::Show,
            MediaKind::Movie => Self::Movie,
        }
    }
}

impl From<SourceName> for SourceNameObject {
    fn from(value: SourceName) -> Self {
        match value {
            SourceName::AniList => Self::Anilist,
            SourceName::MyAnimeList => Self::Myanimelist,
            SourceName::Jikan => Self::Jikan,
            SourceName::Kitsu => Self::Kitsu,
            SourceName::Tvmaze => Self::Tvmaze,
            SourceName::Imdb => Self::Imdb,
        }
    }
}

impl From<animedb::ExternalId> for ExternalIdObject {
    fn from(value: animedb::ExternalId) -> Self {
        Self {
            source: value.source.into(),
            source_id: value.source_id,
            url: value.url,
        }
    }
}

impl From<animedb::SourcePayload> for SourcePayloadObject {
    fn from(value: animedb::SourcePayload) -> Self {
        Self {
            source: value.source.into(),
            source_id: value.source_id,
            url: value.url,
            remote_updated_at: value.remote_updated_at,
            raw_json: value.raw_json.map(|raw| raw.to_string()),
        }
    }
}

async fn graphql_handler(
    State(schema): State<AnimeDbSchema>,
    req: GraphQLRequest,
) -> GraphQLResponse {
    schema.execute(req.into_inner()).await.into()
}

async fn playground() -> impl IntoResponse {
    Html(playground_source(GraphQLPlaygroundConfig::new("/graphql")))
}

async fn healthz(State(schema): State<AnimeDbSchema>) -> impl IntoResponse {
    let ok = tokio::task::spawn_blocking(move || {
        let db = AnimeDb::open(schema.data::<AppState>().unwrap().database_path.as_path())
            .map_err(|e| e.to_string())?;
        db.connection()
            .query_row("SELECT 1", [], |_| Ok(()))
            .map_err(|e| format!("{e}"))
    })
    .await
    .map_err(|e| format!("{e}"))
    .is_ok();

    Json(serde_json::json!({ "status": if ok { "ok" } else { "error" } }))
}

fn not_found_is_none<T>(error: animedb::Error) -> GraphQLResult<Option<T>> {
    match error {
        animedb::Error::NotFound => Ok(None),
        other => Err(async_graphql::Error::new(other.to_string())),
    }
}

fn join_error(error: tokio::task::JoinError) -> async_graphql::Error {
    async_graphql::Error::new(error.to_string())
}

fn search_remote(
    source: SourceNameObject,
    query: &str,
    options: SearchOptions,
) -> animedb::Result<Vec<MediaObject>> {
    match source {
        SourceNameObject::Anilist => {
            let remote = RemoteCatalog::new(AniListProvider::default());
            remote
                .search(query, options)
                .map(|items| items.into_iter().map(MediaObject::from_canonical).collect())
        }
        SourceNameObject::Jikan => {
            let remote = RemoteCatalog::new(JikanProvider::default());
            remote
                .search(query, options)
                .map(|items| items.into_iter().map(MediaObject::from_canonical).collect())
        }
        SourceNameObject::Kitsu => {
            let remote = RemoteCatalog::new(KitsuProvider::default());
            remote
                .search(query, options)
                .map(|items| items.into_iter().map(MediaObject::from_canonical).collect())
        }
        SourceNameObject::Tvmaze => {
            let remote = RemoteCatalog::new(TvmazeProvider::default());
            remote
                .search(query, options)
                .map(|items| items.into_iter().map(MediaObject::from_canonical).collect())
        }
        SourceNameObject::Imdb => Err(animedb::Error::Validation(
            "IMDb remote search requires downloading the full dataset; use sync instead".into(),
        )),
        SourceNameObject::Myanimelist => Err(animedb::Error::Validation(
            "consulta remota direta para MyAnimeList nao esta disponivel".into(),
        )),
    }
}

fn get_remote_media(
    source: SourceNameObject,
    source_id: &str,
    media_kind: MediaKindObject,
) -> animedb::Result<Option<MediaObject>> {
    match source {
        SourceNameObject::Anilist => {
            let remote = RemoteCatalog::new(AniListProvider::default());
            let collection = match media_kind {
                MediaKindObject::Anime => remote.anime_metadata(),
                MediaKindObject::Manga => remote.manga_metadata(),
                MediaKindObject::Show | MediaKindObject::Movie => remote.anime_metadata(),
            };
            collection
                .by_id(source_id)
                .map(|media| media.map(MediaObject::from_canonical))
        }
        SourceNameObject::Jikan => {
            let remote = RemoteCatalog::new(JikanProvider::default());
            let collection = match media_kind {
                MediaKindObject::Anime => remote.anime_metadata(),
                MediaKindObject::Manga => remote.manga_metadata(),
                MediaKindObject::Show | MediaKindObject::Movie => remote.anime_metadata(),
            };
            collection
                .by_id(source_id)
                .map(|media| media.map(MediaObject::from_canonical))
        }
        SourceNameObject::Kitsu => {
            let remote = RemoteCatalog::new(KitsuProvider::default());
            let collection = match media_kind {
                MediaKindObject::Anime => remote.anime_metadata(),
                MediaKindObject::Manga => remote.manga_metadata(),
                MediaKindObject::Show | MediaKindObject::Movie => remote.anime_metadata(),
            };
            collection
                .by_id(source_id)
                .map(|media| media.map(MediaObject::from_canonical))
        }
        SourceNameObject::Tvmaze => {
            let remote = RemoteCatalog::new(TvmazeProvider::default());
            let collection = remote.show_metadata();
            collection
                .by_id(source_id)
                .map(|media| media.map(MediaObject::from_canonical))
        }
        SourceNameObject::Imdb => Err(animedb::Error::Validation(
            "IMDb remote lookup requires downloading the full dataset; use sync instead".into(),
        )),
        SourceNameObject::Myanimelist => Err(animedb::Error::Validation(
            "consulta remota direta para MyAnimeList nao esta disponivel".into(),
        )),
    }
}

fn sync_with_max_pages(db: &mut AnimeDb, max_pages: usize) -> animedb::Result<SyncReport> {
    let anilist = AniListProvider::default();
    let jikan = JikanProvider::default();
    let kitsu = KitsuProvider::default();
    let tvmaze = TvmazeProvider::default();
    let imdb = ImdbProvider::default();
    let mut outcomes = Vec::new();

    for media_kind in [MediaKind::Anime, MediaKind::Manga] {
        outcomes.push(
            db.sync_from(
                &anilist,
                SyncRequest::new(SourceName::AniList)
                    .with_media_kind(media_kind)
                    .with_max_pages(max_pages),
            )?,
        );
        outcomes.push(
            db.sync_from(
                &jikan,
                SyncRequest::new(SourceName::Jikan)
                    .with_media_kind(media_kind)
                    .with_max_pages(max_pages),
            )?,
        );
        outcomes.push(
            db.sync_from(
                &kitsu,
                SyncRequest::new(SourceName::Kitsu)
                    .with_media_kind(media_kind)
                    .with_max_pages(max_pages),
            )?,
        );
    }

    outcomes.push(
        db.sync_from(
            &tvmaze,
            SyncRequest::new(SourceName::Tvmaze)
                .with_media_kind(MediaKind::Show)
                .with_max_pages(max_pages),
        )?,
    );

    for media_kind in [MediaKind::Show, MediaKind::Movie] {
        outcomes.push(
            db.sync_from(
                &imdb,
                SyncRequest::new(SourceName::Imdb)
                    .with_media_kind(media_kind)
                    .with_max_pages(max_pages),
            )?,
        );
    }

    let total_upserted_records = outcomes.iter().map(|item| item.upserted_records).sum();

    Ok(SyncReport {
        outcomes,
        total_upserted_records,
    })
}
