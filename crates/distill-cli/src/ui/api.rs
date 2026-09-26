//! The JSON API behind the web UI. Request and response types are defined here and in
//! `distill-core`; the UI's TypeScript types are generated from them by ts-rs.
//!
//! Every handler opens config, vault and index afresh on a blocking thread, the same way
//! `distill mcp` does, so the server always sees the current vault and settings.

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post, put};
use axum::{Json, Router};
use distill_core::Distill;
use distill_core::embed::Embedder;
use distill_core::index::{
    Conflict, InvalidFile, NoteDetail, NoteHit, RecallResult, SimilarTopic, Stats, TagCount,
    TopicCount, TopicDetail,
};
use distill_core::ops::Duplicates;
use serde::{Deserialize, Serialize};

use super::server::AppState;

#[derive(Debug, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct Health {
    /// Always `"distill"`: tells a Distill server apart from another program on the port.
    pub app: String,
    pub version: String,
    pub pid: u32,
}

#[derive(Debug, Serialize, ts_rs::TS)]
#[ts(export)]
pub struct Session {
    pub version: String,
    pub vault: String,
    /// `http://distill.localhost:<port>`, the address links point at.
    pub origin: String,
    /// Whether `distill model pull` has run; enables similar-topic suggestions.
    pub embedding_model: bool,
}

#[derive(Debug, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct NotesQuery {
    /// Keywords; empty lists the most recent notes.
    #[ts(optional)]
    pub q: Option<String>,
    #[ts(optional)]
    pub tag: Option<String>,
    #[ts(optional)]
    pub limit: Option<usize>,
}

#[derive(Debug, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct RecallQuery {
    pub q: String,
    #[ts(optional)]
    pub limit: Option<usize>,
}

#[derive(Debug, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct AnnotationInput {
    pub text: String,
}

#[derive(Debug, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct TopicRename {
    pub label: String,
}

#[derive(Debug, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct TopicMerge {
    /// The topic to merge into. The topic in the URL is the one that goes away.
    pub into: String,
}

#[derive(Debug, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct PairDismissal {
    pub a: String,
    pub b: String,
}

#[derive(Debug, Serialize, ts_rs::TS)]
#[ts(export)]
pub struct Merged {
    /// The topic both now resolve to.
    pub topic_id: String,
}

#[derive(Debug, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct ConflictChoice {
    /// `note`, `topic` or `annotation`.
    pub kind: String,
    pub id: String,
    /// Vault-relative path of the copy to keep.
    pub keep: String,
}

#[derive(Debug, Serialize, ts_rs::TS)]
#[ts(export)]
pub struct Problems {
    pub conflicts: Vec<Conflict>,
    pub invalid_files: Vec<InvalidFile>,
}

#[derive(Debug, Serialize, ts_rs::TS)]
#[ts(export)]
pub struct ApiErrorBody {
    pub error: String,
}

const DEFAULT_LIMIT: usize = 50;
const SIMILAR_LIMIT: usize = 5;
const MAX_LIMIT: usize = 500;

/// Routes under `/api`, all behind the session check in `server.rs`.
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/session", get(session))
        .route("/notes", get(notes))
        .route("/notes/{id}", get(note))
        .route("/notes/{id}/annotations", post(add_annotation))
        .route(
            "/annotations/{id}",
            put(update_annotation).delete(delete_annotation),
        )
        .route("/topics", get(topics))
        .route("/topics/{id}", get(topic).put(rename_topic))
        .route("/topics/{id}/merge", post(merge_topic))
        .route("/topics/{id}/similar", get(similar_topics))
        .route("/duplicates", get(duplicates))
        .route("/duplicates/dismiss", post(dismiss_duplicate))
        .route("/tags", get(tags))
        .route("/stats", get(stats))
        .route("/recall", get(recall))
        .route("/problems", get(problems))
        .route("/conflicts/resolve", post(resolve_conflict))
}

pub fn health() -> Health {
    Health {
        app: "distill".into(),
        version: env!("CARGO_PKG_VERSION").into(),
        pid: std::process::id(),
    }
}

async fn session(State(s): State<AppState>) -> Result<Json<Session>, ApiError> {
    read(&s, |d| {
        Ok(Session {
            version: env!("CARGO_PKG_VERSION").into(),
            vault: d.vault.root().display().to_string(),
            origin: d.config.ui_origin(),
            embedding_model: Embedder::installed(&d.dirs),
        })
    })
    .await
}

async fn notes(
    State(s): State<AppState>,
    Query(q): Query<NotesQuery>,
) -> Result<Json<Vec<NoteHit>>, ApiError> {
    let limit = q.limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT);
    read(&s, move |d| {
        d.index
            .search(q.q.as_deref().unwrap_or_default(), q.tag.as_deref(), limit)
    })
    .await
}

async fn note(
    State(s): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<NoteDetail>, ApiError> {
    read(&s, move |d| {
        d.index
            .note_detail(&id)?
            .ok_or(distill_core::Error::UnknownNote { id })
    })
    .await
}

async fn add_annotation(
    State(s): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<AnnotationInput>,
) -> Result<Json<NoteDetail>, ApiError> {
    let detail = write(&s, move |d| {
        d.annotate(&id, &body.text)?;
        d.index
            .note_detail(&id)?
            .ok_or(distill_core::Error::UnknownNote { id })
    })
    .await?;
    Ok(Json(detail))
}

async fn update_annotation(
    State(s): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<AnnotationInput>,
) -> Result<StatusCode, ApiError> {
    write(&s, move |d| d.update_annotation(&id, &body.text)).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn delete_annotation(
    State(s): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    write(&s, move |d| d.delete_annotation(&id)).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn topics(State(s): State<AppState>) -> Result<Json<Vec<TopicCount>>, ApiError> {
    read(&s, |d| d.index.topics()).await
}

async fn topic(
    State(s): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<TopicDetail>, ApiError> {
    read(&s, move |d| {
        d.index
            .topic_detail(&id)?
            .ok_or(distill_core::Error::UnknownTopic { id })
    })
    .await
}

async fn rename_topic(
    State(s): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<TopicRename>,
) -> Result<StatusCode, ApiError> {
    write(&s, move |d| d.rename_topic(&id, &body.label)).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn merge_topic(
    State(s): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<TopicMerge>,
) -> Result<Json<Merged>, ApiError> {
    let merged = write(&s, move |d| {
        Ok(Merged {
            topic_id: d.merge_topic(&id, &body.into)?,
        })
    })
    .await?;
    Ok(Json(merged))
}

async fn similar_topics(
    State(s): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Vec<SimilarTopic>>, ApiError> {
    read(&s, move |d| d.similar_topics(&id, SIMILAR_LIMIT)).await
}

async fn duplicates(State(s): State<AppState>) -> Result<Json<Duplicates>, ApiError> {
    read(&s, |d| d.duplicate_topics()).await
}

async fn dismiss_duplicate(
    State(s): State<AppState>,
    Json(body): Json<PairDismissal>,
) -> Result<StatusCode, ApiError> {
    write(&s, move |d| d.index.dismiss_pair(&body.a, &body.b)).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn tags(State(s): State<AppState>) -> Result<Json<Vec<TagCount>>, ApiError> {
    read(&s, |d| d.index.tags()).await
}

async fn stats(State(s): State<AppState>) -> Result<Json<Stats>, ApiError> {
    read(&s, |d| d.index.stats()).await
}

async fn recall(
    State(s): State<AppState>,
    Query(q): Query<RecallQuery>,
) -> Result<Json<RecallResult>, ApiError> {
    read(&s, move |d| d.recall(&q.q, q.limit.unwrap_or(5))).await
}

async fn problems(State(s): State<AppState>) -> Result<Json<Problems>, ApiError> {
    read(&s, |d| {
        Ok(Problems {
            conflicts: d.index.conflicts()?,
            invalid_files: d.index.invalid_files()?,
        })
    })
    .await
}

async fn resolve_conflict(
    State(s): State<AppState>,
    Json(body): Json<ConflictChoice>,
) -> Result<StatusCode, ApiError> {
    write(&s, move |d| {
        d.resolve_conflict(&body.kind, &body.id, &body.keep)
            .map(drop)
    })
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn read<T, F>(state: &AppState, f: F) -> Result<Json<T>, ApiError>
where
    T: Send + 'static,
    F: FnOnce(&mut Distill) -> distill_core::Result<T> + Send + 'static,
{
    Ok(Json(run(state, f).await?))
}

/// Like `read`, then tells open pages that the vault changed.
async fn write<T, F>(state: &AppState, f: F) -> Result<T, ApiError>
where
    T: Send + 'static,
    F: FnOnce(&mut Distill) -> distill_core::Result<T> + Send + 'static,
{
    let out = run(state, f).await?;
    state.notify_changed();
    Ok(out)
}

async fn run<T, F>(state: &AppState, f: F) -> Result<T, ApiError>
where
    T: Send + 'static,
    F: FnOnce(&mut Distill) -> distill_core::Result<T> + Send + 'static,
{
    let dirs = state.dirs.clone();
    Ok(tokio::task::spawn_blocking(move || {
        let mut d = Distill::open_in(dirs)?;
        f(&mut d)
    })
    .await
    .map_err(|e| ApiError::internal(format!("request task failed: {e}")))??)
}

#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    pub fn new(status: StatusCode, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }

    fn internal(message: String) -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, message)
    }
}

impl From<distill_core::Error> for ApiError {
    fn from(err: distill_core::Error) -> Self {
        use distill_core::Error as E;
        let status = match &err {
            E::UnknownNote { .. }
            | E::UnknownTopic { .. }
            | E::UnknownAnnotation { .. }
            | E::UnknownConflict { .. } => StatusCode::NOT_FOUND,
            E::InvalidTopic { .. }
            | E::InvalidMerge { .. }
            | E::NotAConflictCopy { .. }
            | E::InvalidNote { .. } => StatusCode::BAD_REQUEST,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };
        let mut message = err.to_string();
        let mut source = std::error::Error::source(&err);
        while let Some(cause) = source {
            message.push_str(&format!(": {cause}"));
            source = cause.source();
        }
        if status == StatusCode::INTERNAL_SERVER_ERROR {
            eprintln!("distill ui: {message}");
        }
        Self { status, message }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(ApiErrorBody {
                error: self.message,
            }),
        )
            .into_response()
    }
}
