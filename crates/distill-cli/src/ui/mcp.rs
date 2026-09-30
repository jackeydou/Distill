//! The same API handlers over MCP, without HTTP authentication or a loopback server.

use anyhow::{Context, Result, ensure};
use axum::body::{Body, to_bytes};
use axum::http::{Method, Request, Uri, header};
use axum::{Json, Router};
use distill_core::config::{Dirs, LocalConfig};
use serde::Serialize;
use serde_json::{Value, json};
use tower::ServiceExt;

use super::{api, server::AppState};

#[derive(Serialize, ts_rs::TS)]
#[ts(export)]
pub struct McpResponse {
    pub status: u16,
    #[ts(type = "unknown")]
    pub data: Value,
}

pub async fn request(method: Method, path: &str, body: Option<Value>) -> Result<McpResponse> {
    let uri: Uri = path
        .parse()
        .with_context(|| format!("invalid UI path {path}"))?;
    ensure!(
        path.starts_with('/')
            && !path.starts_with("//")
            && uri.scheme().is_none()
            && uri.authority().is_none(),
        "UI path {path} must be an absolute API path, such as /notes"
    );
    let dirs = Dirs::discover()?;
    let port = LocalConfig::load(&dirs)?.ui.port;
    let state = AppState::new(dirs, port)?;
    let app = api::routes()
        .fallback(|| async {
            (
                axum::http::StatusCode::NOT_FOUND,
                Json(json!({"error": "unknown UI API path"})),
            )
        })
        .method_not_allowed_fallback(|| async {
            (
                axum::http::StatusCode::METHOD_NOT_ALLOWED,
                Json(json!({"error": "method not allowed for UI API path"})),
            )
        })
        .with_state(state);
    let app: Router = app;
    let body = match body {
        Some(value) => Body::from(serde_json::to_vec(&value)?),
        None => Body::empty(),
    };
    let response = app
        .oneshot(
            Request::builder()
                .method(method)
                .uri(uri)
                .header(header::CONTENT_TYPE, "application/json")
                .body(body)?,
        )
        .await?;
    let status = response.status().as_u16();
    let bytes = to_bytes(response.into_body(), 16 * 1024 * 1024).await?;
    let data = if bytes.is_empty() {
        Value::Null
    } else {
        // Axum's request extractors return plain text for malformed JSON or query values.
        match serde_json::from_slice(&bytes) {
            Ok(value) => value,
            Err(_) if status >= 400 => json!({"error": String::from_utf8(bytes.to_vec())?}),
            Err(error) => return Err(error).context("UI API returned invalid JSON"),
        }
    };
    Ok(McpResponse { status, data })
}
