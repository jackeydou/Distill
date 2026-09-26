//! The HTTP server behind `distill ui`: the embedded web app, the JSON API, and a
//! server-sent event stream that tells open pages when the vault changed.
//!
//! It listens on loopback only (`127.0.0.1` and `::1`), accepts only loopback host names
//! (against DNS rebinding), serves no CORS headers, and requires the session secret on every
//! API call (spec D7).

use std::convert::Infallible;
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr};
use std::path::Path;
use std::time::Duration;

use anyhow::{Context, Result};
use axum::body::Body;
use axum::extract::{Query, Request, State};
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode, Uri, header};
use axum::middleware::{self, Next};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use distill_core::config::{Dirs, UI_HOST};
use notify_debouncer_mini::{DebounceEventResult, new_debouncer};
use serde::Deserialize;
use tokio::net::TcpListener;
use tokio::sync::{broadcast, watch};
use tokio_stream::wrappers::{BroadcastStream, WatchStream};
use tokio_stream::{Stream, StreamExt};

use super::api::{self, ApiError};
use super::auth::{self, Auth};

#[derive(Clone)]
pub struct AppState {
    pub dirs: Dirs,
    pub auth: Auth,
    pub port: u16,
    changes: broadcast::Sender<()>,
    shutdown: watch::Sender<bool>,
}

impl AppState {
    pub fn notify_changed(&self) {
        // No receivers just means no page is open.
        let _ = self.changes.send(());
    }
}

#[derive(rust_embed::Embed)]
#[folder = "$CARGO_MANIFEST_DIR/../../apps/web/dist/"]
#[allow_missing = true]
struct Assets;

pub struct Bound {
    listeners: Vec<TcpListener>,
}

/// Binds the port on both loopback addresses. `::1` is skipped when the machine has no
/// IPv6 loopback; `distill.localhost` then resolves to `127.0.0.1` in browsers.
pub async fn bind(port: u16) -> std::io::Result<Bound> {
    let v4 = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, port))).await?;
    let mut listeners = vec![v4];
    match TcpListener::bind(SocketAddr::from((Ipv6Addr::LOCALHOST, port))).await {
        Ok(v6) => listeners.push(v6),
        Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => return Err(e),
        Err(e) => {
            eprintln!("distill ui: not listening on [::1]:{port} ({e}); using 127.0.0.1 only")
        }
    }
    Ok(Bound { listeners })
}

/// Serves until Ctrl-C or `POST /api/shutdown`.
pub async fn serve(bound: Bound, dirs: Dirs, vault: &Path, port: u16) -> Result<()> {
    let auth = Auth::load_or_create(&dirs.data_dir.join("ui"))?;
    let (changes, _) = broadcast::channel(16);
    let (shutdown, _) = watch::channel(false);
    let state = AppState {
        dirs,
        auth,
        port,
        changes,
        shutdown,
    };
    let _watcher = watch_vault(vault, state.clone())?;
    let app = router(state.clone());

    let mut servers = tokio::task::JoinSet::new();
    for listener in bound.listeners {
        let app = app.clone();
        let mut stop = state.shutdown.subscribe();
        servers.spawn(async move {
            axum::serve(listener, app)
                .with_graceful_shutdown(async move {
                    let _ = stop.wait_for(|s| *s).await;
                })
                .await
        });
    }
    let trigger = state.shutdown.clone();
    tokio::spawn(async move {
        if tokio::signal::ctrl_c().await.is_ok() {
            trigger.send_replace(true);
        }
    });
    while let Some(done) = servers.join_next().await {
        done.context("server task panicked")?
            .context("server stopped with an error")?;
    }
    Ok(())
}

fn router(state: AppState) -> Router {
    let api = api::routes()
        .route("/events", get(events))
        .route("/shutdown", post(shutdown))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            require_session,
        ));
    Router::new()
        .route("/api/health", get(|| async { Json(api::health()) }))
        .route("/auth", get(authorize))
        .nest("/api", api)
        .fallback(get(asset))
        .layer(middleware::from_fn_with_state(state.clone(), check_origin))
        .with_state(state)
}

/// Rejects requests whose `Host` is not a loopback name (DNS rebinding) and requests sent
/// by a page on another origin. A cross-origin page could otherwise post to the API with
/// the cookie, because every `*.localhost` port counts as the same site.
async fn check_origin(State(s): State<AppState>, req: Request, next: Next) -> Response {
    let headers = req.headers();
    let host_ok = headers
        .get(header::HOST)
        .and_then(|h| h.to_str().ok())
        .is_some_and(|h| allowed_host(h, None));
    if !host_ok {
        return forbidden("unknown Host; open Distill at http://distill.localhost");
    }
    let origin = headers.get(header::ORIGIN).and_then(|o| o.to_str().ok());
    let origin_ok = match origin {
        Some(o) => o
            .strip_prefix("http://")
            .is_some_and(|h| allowed_host(h, Some(s.port))),
        // Browsers send Origin with every request that can change something.
        None => matches!(*req.method(), Method::GET | Method::HEAD) || bearer(headers).is_some(),
    };
    if !origin_ok {
        return forbidden("cross-origin request refused");
    }
    next.run(req).await
}

fn allowed_host(host: &str, port: Option<u16>) -> bool {
    let (name, host_port) = match host.rsplit_once(':') {
        Some((name, p)) if !name.is_empty() && !p.contains(']') => (name, p.parse::<u16>().ok()),
        _ => (host, None),
    };
    let name_ok = matches!(name, UI_HOST | "localhost" | "127.0.0.1" | "[::1]");
    name_ok && port.is_none_or(|p| host_port == Some(p))
}

async fn require_session(State(s): State<AppState>, req: Request, next: Next) -> Response {
    let headers = req.headers();
    let presented = bearer(headers).or_else(|| {
        headers
            .get(header::COOKIE)
            .and_then(|c| c.to_str().ok())
            .and_then(|c| auth::cookie_value(c, auth::COOKIE))
    });
    if !presented.is_some_and(|p| s.auth.check(p)) {
        return ApiError::new(
            StatusCode::UNAUTHORIZED,
            "this browser is not authorized. Run `distill ui` in a terminal to open an authorized page.",
        )
        .into_response();
    }
    next.run(req).await
}

fn bearer(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(header::AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
}

fn forbidden(message: &str) -> Response {
    ApiError::new(StatusCode::FORBIDDEN, message).into_response()
}

#[derive(Deserialize)]
struct AuthQuery {
    token: String,
    next: Option<String>,
}

/// Redeems a one-time token from `distill ui` and sets the session cookie.
async fn authorize(State(s): State<AppState>, Query(q): Query<AuthQuery>) -> Response {
    let next = q
        .next
        .filter(|n| n.starts_with('/') && !n.starts_with("//"))
        .unwrap_or_else(|| "/".into());
    match s.auth.redeem(&q.token) {
        Ok(true) => {
            let mut res = Redirect::to(&next).into_response();
            if let Ok(cookie) = HeaderValue::from_str(&s.auth.set_cookie()) {
                res.headers_mut().insert(header::SET_COOKIE, cookie);
            }
            res
        }
        Ok(false) => (
            StatusCode::FORBIDDEN,
            "This link has expired or was already used. Run `distill ui` again to get a new one.",
        )
            .into_response(),
        Err(e) => {
            ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, format!("{e:#}")).into_response()
        }
    }
}

async fn shutdown(State(s): State<AppState>, headers: HeaderMap) -> Response {
    if bearer(&headers).is_none() {
        return forbidden("shutdown is only available to `distill ui stop`");
    }
    s.shutdown.send_replace(true);
    StatusCode::NO_CONTENT.into_response()
}

/// One `changed` event whenever the vault changes; ends when the server shuts down so
/// graceful shutdown does not wait on open pages.
async fn events(State(s): State<AppState>) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let changes = BroadcastStream::new(s.changes.subscribe())
        .map(|_| Some(Event::default().event("changed").data("{}")));
    let stop = WatchStream::new(s.shutdown.subscribe())
        .filter(|stopping| *stopping)
        .map(|_| None);
    let stream = changes.merge(stop).map_while(|e| e.map(Ok));
    Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
}

/// The web app. Hashed files under `assets/` are cached forever; any other path without a
/// file extension is a client-side route and gets `index.html`.
async fn asset(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    if let Some(file) = Assets::get(path).filter(|_| !path.is_empty()) {
        let cache = if path.starts_with("assets/") {
            "public, max-age=31536000, immutable"
        } else {
            "no-cache"
        };
        return file_response(&file, cache);
    }
    if path
        .rsplit('/')
        .next()
        .is_some_and(|last| last.contains('.'))
    {
        return (StatusCode::NOT_FOUND, "not found").into_response();
    }
    match Assets::get("index.html") {
        Some(index) => file_response(&index, "no-cache"),
        None => (
            StatusCode::SERVICE_UNAVAILABLE,
            "This distill was built without the web UI. Run `pnpm -C apps/web build`, then rebuild distill.",
        )
            .into_response(),
    }
}

fn file_response(file: &rust_embed::EmbeddedFile, cache: &'static str) -> Response {
    let mut res = Response::new(Body::from(file.data.clone().into_owned()));
    let headers = res.headers_mut();
    if let Ok(mime) = HeaderValue::from_str(file.metadata.mimetype()) {
        headers.insert(header::CONTENT_TYPE, mime);
    }
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static(cache));
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    res
}

/// Watches the vault for changes from other devices, editors and processes. Paths starting
/// with `.` (sync-tool metadata, Distill's temp files) are ignored.
fn watch_vault(
    vault: &Path,
    state: AppState,
) -> Result<notify_debouncer_mini::Debouncer<notify_debouncer_mini::notify::RecommendedWatcher>> {
    let root = vault.to_path_buf();
    let mut debouncer = new_debouncer(
        Duration::from_millis(300),
        move |res: DebounceEventResult| match res {
            Ok(events) => {
                let relevant = events.iter().any(|e| {
                    e.path.strip_prefix(&root).is_ok_and(|rel| {
                        !rel.components()
                            .any(|c| c.as_os_str().to_string_lossy().starts_with('.'))
                    })
                });
                if relevant {
                    state.notify_changed();
                }
            }
            Err(e) => eprintln!("distill ui: file watcher error: {e}"),
        },
    )
    .context("starting the vault file watcher")?;
    debouncer
        .watcher()
        .watch(
            vault,
            notify_debouncer_mini::notify::RecursiveMode::Recursive,
        )
        .with_context(|| format!("watching {}", vault.display()))?;
    Ok(debouncer)
}

#[cfg(test)]
mod tests {
    use super::allowed_host;

    #[test]
    fn host_names() {
        assert!(allowed_host("distill.localhost:4777", None));
        assert!(allowed_host("127.0.0.1:4777", Some(4777)));
        assert!(allowed_host("[::1]:4777", Some(4777)));
        assert!(allowed_host("localhost", None));
        assert!(!allowed_host("evil.com:4777", None));
        assert!(!allowed_host("distill.localhost.evil.com", None));
        assert!(!allowed_host("other.localhost:4777", None));
        assert!(!allowed_host("distill.localhost:3000", Some(4777)));
    }
}
