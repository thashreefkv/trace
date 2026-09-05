//! Remote HTTP API for Siri / Apple Shortcuts.
//!
//! Runs an `axum` server inside the Trace process, bound to the Tailscale
//! interface (and 127.0.0.1) only. Two independent locks gate access:
//!
//!   1. **Network**: only hosts on the same tailnet can reach the bind
//!      address. We pick the local 100.64.0.0/10 address via
//!      `project_manager_shared::tailscale::tailscale_ipv4()`. If Tailscale
//!      isn't running we refuse to start the server at all rather than
//!      silently falling back to a less-private bind.
//!
//!   2. **Process**: every request must carry `Authorization: Bearer <token>`
//!      matching the value held in macOS Keychain. The token is created on
//!      first run and can be regenerated from Settings.
//!
//! All routes share the live `SqlitePool` and `brain_path` from the rest of
//! the app, so an Ask call here exercises the exact same agent loop as the
//! in-app Ask Workspace.
//!
//! The `lib.rs` setup hook spawns `start()` via `tauri::async_runtime::spawn`
//! per the project-wide async rule.

use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::PathBuf,
    sync::Arc,
};

use axum::{
    extract::{DefaultBodyLimit, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::SqlitePool;
use tokio::sync::RwLock;

use project_manager_shared::{keychain, models::AskSearchResult, tailscale};

/// Port the HTTP API listens on. Picked to be high enough to avoid clashing
/// with common dev ports, low enough to be easy to remember. Overridable via
/// the `TRACE_SIRI_PORT` env var so tests can pick an ephemeral port.
const DEFAULT_PORT: u16 = 8421;
const MAX_REQUEST_BODY_BYTES: usize = 64 * 1024;
const MAX_QUESTION_CHARS: usize = 8_000;
const MAX_CONTEXT_CHARS: usize = 24_000;
const MAX_CAPTURE_CHARS: usize = 10_000;

/// Shared state passed to every handler. Cloning is cheap — every field is
/// either an `Arc`, a `SqlitePool` (which is already an `Arc` internally),
/// or a `PathBuf`.
#[derive(Clone)]
pub struct HttpApiState {
    pub pool: SqlitePool,
    pub brain_path: PathBuf,
    pub app_support_dir: PathBuf,
    /// In-memory cache of the bearer token. Held in an `RwLock` so a
    /// "regenerate" call from the UI can swap it atomically without
    /// dropping any in-flight requests.
    pub token: Arc<RwLock<String>>,
}

impl HttpApiState {
    pub fn new(
        pool: SqlitePool,
        brain_path: PathBuf,
        app_support_dir: PathBuf,
        token: String,
    ) -> Self {
        Self {
            pool,
            brain_path,
            app_support_dir,
            token: Arc::new(RwLock::new(token)),
        }
    }

    /// Replace the in-memory token. Callers should call this after saving the
    /// new value to the platform credential store so the two stay in sync.
    pub async fn set_token(&self, new_token: String) {
        let mut guard = self.token.write().await;
        *guard = new_token;
    }
}

/// Snapshot of where the API is reachable. Used by Settings to render the
/// URL the user pastes into the Shortcut.
#[derive(Debug, Clone, Serialize)]
pub struct HttpApiStatus {
    pub running: bool,
    pub port: u16,
    pub tailscale_ipv4: Option<String>,
    pub tailscale_url: Option<String>,
    pub localhost_url: String,
}

/// Decide where the server should bind and produce a status snapshot. Returns
/// `None` for `tailscale_*` fields when Tailscale isn't running — callers
/// should NOT start the server in that case.
pub fn detect_status() -> HttpApiStatus {
    let port = port();
    let tailscale_ipv4 = tailscale::tailscale_ipv4();
    HttpApiStatus {
        running: false, // Filled in by start() once we successfully bind.
        port,
        tailscale_ipv4: tailscale_ipv4.map(|ip| ip.to_string()),
        tailscale_url: tailscale_ipv4.map(|ip| format!("http://{ip}:{port}")),
        localhost_url: format!("http://127.0.0.1:{port}"),
    }
}

fn port() -> u16 {
    std::env::var("TRACE_SIRI_PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_PORT)
}

/// Start the HTTP server. Spawns two listener tasks — one on the Tailscale
/// interface, one on localhost — using `tauri::async_runtime::spawn`.
///
/// Returns `Ok(())` immediately after spawning; the listeners run forever.
/// If Tailscale isn't detected we log and return without spawning anything,
/// so the rest of the app keeps running normally.
pub async fn start(state: HttpApiState) -> Result<(), String> {
    let Some(ts_ip) = tailscale::tailscale_ipv4() else {
        eprintln!(
            "[siri-api] Tailscale not detected — remote API will not start. \
             Install/sign-in to Tailscale and restart Trace to enable Siri access."
        );
        return Ok(());
    };

    let port = port();
    let app = build_router(state.clone());

    spawn_listener(
        app.clone(),
        SocketAddr::new(IpAddr::V4(ts_ip), port),
        "tailscale",
    );
    spawn_listener(
        app,
        SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port),
        "localhost",
    );

    eprintln!("[siri-api] listening on http://{ts_ip}:{port} and http://127.0.0.1:{port}");
    Ok(())
}

fn spawn_listener(app: Router, addr: SocketAddr, label: &'static str) {
    tauri::async_runtime::spawn(async move {
        match tokio::net::TcpListener::bind(addr).await {
            Ok(listener) => {
                if let Err(error) = axum::serve(listener, app).await {
                    eprintln!("[siri-api] {label} listener stopped: {error}");
                }
            }
            Err(error) => {
                eprintln!("[siri-api] failed to bind {label} ({addr}): {error}");
            }
        }
    });
}

fn build_router(state: HttpApiState) -> Router {
    Router::new()
        .route("/health", get(health))
        // POST /ask + POST /capture are added in the routes below.
        .route("/ask", post(ask))
        .route("/capture", post(capture))
        .layer(DefaultBodyLimit::max(MAX_REQUEST_BODY_BYTES))
        .layer(middleware::from_fn_with_state(state.clone(), auth))
        .layer(middleware::from_fn(security_headers))
        .with_state(state)
}

async fn security_headers(request: axum::extract::Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static("default-src 'none'; frame-ancestors 'none'"),
    );
    response
}

// ---------------------------------------------------------------------------
// Auth middleware

async fn auth(
    State(state): State<HttpApiState>,
    headers: HeaderMap,
    request: axum::extract::Request,
    next: Next,
) -> Response {
    let presented = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "));

    let expected = state.token.read().await;
    let ok = match presented {
        Some(p) => constant_time_eq(p.as_bytes(), expected.as_bytes()),
        None => false,
    };
    drop(expected);

    if !ok {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({ "error": "unauthorized" })),
        )
            .into_response();
    }

    next.run(request).await
}

/// Length-checking constant-time byte comparison. Returns false immediately
/// if the lengths differ (which leaks length, not token bytes — acceptable
/// since the token length is fixed and public). Otherwise XOR-or-fold across
/// every byte so the loop always touches every position.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff: u8 = 0;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

// ---------------------------------------------------------------------------
// Routes

#[derive(Debug, Serialize)]
struct HealthResponse {
    ok: bool,
    version: &'static str,
    tailscale: bool,
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        ok: true,
        version: env!("CARGO_PKG_VERSION"),
        tailscale: tailscale::tailscale_ipv4().is_some(),
    })
}

// ---------------------------------------------------------------------------
// POST /ask

#[derive(Debug, Deserialize)]
struct AskRequest {
    question: String,
    #[serde(default)]
    context: Option<String>,
}

#[derive(Debug, Serialize)]
struct AskResponse {
    answer: String,
    refs: Vec<project_manager_shared::models::SearchResult>,
    retrieval_query: Option<String>,
    took_ms: u128,
}

async fn ask(
    State(state): State<HttpApiState>,
    Json(req): Json<AskRequest>,
) -> Result<Json<AskResponse>, ApiError> {
    let question = req.question.trim();
    if question.is_empty() {
        return Err(ApiError::bad_request("`question` must not be empty"));
    }
    if question.chars().count() > MAX_QUESTION_CHARS {
        return Err(ApiError::bad_request("`question` is too long"));
    }
    if req
        .context
        .as_deref()
        .is_some_and(|context| context.chars().count() > MAX_CONTEXT_CHARS)
    {
        return Err(ApiError::bad_request("`context` is too long"));
    }

    let api_key = keychain::get_gemini_api_key(&state.app_support_dir)
        .map_err(ApiError::internal)?
        .ok_or_else(|| {
            ApiError::bad_request(
                "Gemini API key not configured. Open Trace → Settings to add one.",
            )
        })?;

    let started = std::time::Instant::now();
    let result: AskSearchResult = project_manager_shared::gemini::ask_search(
        &api_key,
        question,
        req.context.as_deref(),
        &state.pool,
        Some(&state.brain_path),
        None, // progress events go to the in-app channel only; Siri uses one-shot
    )
    .await
    .map_err(ApiError::internal)?;

    Ok(Json(AskResponse {
        answer: result.answer,
        refs: result.refs,
        retrieval_query: result.retrieval_query,
        took_ms: started.elapsed().as_millis(),
    }))
}

// ---------------------------------------------------------------------------
// POST /capture

#[derive(Debug, Deserialize)]
struct CaptureRequest {
    body: String,
    /// Optional. Defaults to "thought". Anything else is rejected for now
    /// — remote callers shouldn't be able to mint Claude/artifact links
    /// without going through the linker.
    #[serde(default)]
    kind: Option<String>,
}

#[derive(Debug, Serialize)]
struct CaptureResponse {
    id: String,
    kind: String,
    body: String,
    created_at: String,
}

async fn capture(
    State(state): State<HttpApiState>,
    Json(req): Json<CaptureRequest>,
) -> Result<Json<CaptureResponse>, ApiError> {
    let body = req.body.trim();
    if body.is_empty() {
        return Err(ApiError::bad_request("`body` must not be empty"));
    }
    if body.chars().count() > MAX_CAPTURE_CHARS {
        return Err(ApiError::bad_request("`body` is too long"));
    }
    let kind_str = req.kind.as_deref().unwrap_or("thought");
    if kind_str != "thought" {
        return Err(ApiError::bad_request(
            "Only `kind: thought` is allowed via the remote API.",
        ));
    }

    let capture = project_manager_shared::repo::create_capture(
        &state.pool,
        project_manager_shared::models::CreateCaptureInput {
            kind: project_manager_shared::models::CaptureKind::Thought,
            body: body.to_string(),
        },
    )
    .await
    .map_err(ApiError::internal)?;

    // Brain rebuild is fire-and-forget elsewhere in the codebase. We don't
    // wire it here because the in-app brain worker picks up changes on its
    // next tick; doing it inline would slow the Siri response down for no
    // user-visible benefit.

    Ok(Json(CaptureResponse {
        id: capture.id,
        kind: capture.kind,
        body: capture.body,
        created_at: capture.created_at,
    }))
}

// ---------------------------------------------------------------------------
// Error type — turns repo / gemini string errors into clean HTTP responses.

struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: message.into(),
        }
    }

    fn internal(message: impl Into<String>) -> Self {
        eprintln!("[siri-api] internal request error: {}", message.into());
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: "internal server error".to_string(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status, Json(json!({ "error": self.message }))).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constant_time_eq_matches_equal_inputs() {
        assert!(constant_time_eq(b"abc", b"abc"));
        assert!(constant_time_eq(b"", b""));
    }

    #[test]
    fn constant_time_eq_rejects_different_inputs() {
        assert!(!constant_time_eq(b"abc", b"abd"));
        assert!(!constant_time_eq(b"abc", b"ab"));
        assert!(!constant_time_eq(b"abc", b""));
    }
}
