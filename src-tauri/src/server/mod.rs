//! Embedded LAN server (HTTP + WebSocket) that students connect to.
//! It exposes only what students need; every teacher operation stays on Tauri IPC.

pub mod hub;

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::{Arc, RwLock};

use axum::extract::{ConnectInfo, DefaultBodyLimit, State, WebSocketUpgrade};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::sync::{oneshot, Mutex};
use tokio::task::JoinHandle;
use tower_http::limit::RequestBodyLimitLayer;

use crate::config;
use crate::database::Database;
use crate::errors::{AppError, AppResult};
use crate::server::hub::Hub;
use crate::websocket::handler;

/// What the teacher UI shows about the server. Never contains secrets.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerStatus {
    pub running: bool,
    pub ip: String,
    pub port: u16,
    pub discovery: bool,
    /// Last start failure, shown to the teacher (e.g. "port already in use").
    pub error: Option<String>,
}

pub type SharedStatus = Arc<RwLock<ServerStatus>>;

pub fn new_status() -> SharedStatus {
    Arc::new(RwLock::new(ServerStatus::default()))
}

#[derive(Clone)]
pub struct AppState {
    pub db: Database,
    pub hub: Arc<Hub>,
}

struct Running {
    shutdown: oneshot::Sender<()>,
    join: JoinHandle<()>,
    sweeper: JoinHandle<()>,
    mdns: Option<crate::networking::mdns::Advertiser>,
}

pub struct LanServer {
    state: AppState,
    status: SharedStatus,
    running: Mutex<Option<Running>>,
}

impl LanServer {
    pub fn new(db: Database, hub: Arc<Hub>, status: SharedStatus) -> Self {
        Self { state: AppState { db, hub }, status, running: Mutex::new(None) }
    }

    pub fn status(&self) -> ServerStatus {
        self.status.read().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// Binds and serves. `port` 0 picks a free port (used by tests). Replaces a running server.
    pub async fn start(&self, ip: Ipv4Addr, port: u16, advertise: bool) -> AppResult<ServerStatus> {
        self.stop().await;
        let listener = match tokio::net::TcpListener::bind(SocketAddr::new(IpAddr::V4(ip), port)).await {
            Ok(l) => l,
            Err(e) => {
                let msg = if e.kind() == std::io::ErrorKind::AddrInUse {
                    format!("Port {port} is already in use. Pick another port in Settings → Network.")
                } else {
                    format!("Could not listen on {ip}:{port} ({e}).")
                };
                self.set_status(|s| *s = ServerStatus { running: false, ip: ip.to_string(), port, discovery: false, error: Some(msg.clone()) });
                return Err(AppError::Conflict(msg));
            }
        };
        let local = listener.local_addr().map_err(AppError::Io)?;
        let (tx, rx) = oneshot::channel::<()>();
        let app = router(self.state.clone());
        let join = tokio::spawn(async move {
            let serve = axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>())
                .with_graceful_shutdown(async move {
                    let _ = rx.await;
                });
            if let Err(e) = serve.await {
                tracing::error!(error = %e, "LAN server stopped unexpectedly");
            }
        });
        // Deadline watcher: announces time_up and auto-submits when an exam's clock runs out.
        let (db, hub) = (self.state.db.clone(), self.state.hub.clone());
        let sweeper = tokio::spawn(async move {
            let mut notified = std::collections::HashSet::new();
            let mut tick = tokio::time::interval(config::EXPIRY_SWEEP_INTERVAL);
            tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                tick.tick().await;
                if let Err(e) = crate::services::exam_engine::expire_due(&db, &hub, &mut notified).await {
                    tracing::error!(error = %e, "expiry sweep failed");
                }
            }
        });
        // Discovery is a convenience only; failing to advertise never stops the server.
        let mdns = if advertise { crate::networking::mdns::Advertiser::start(ip, local.port()) } else { None };
        let discovery = mdns.is_some();
        *self.running.lock().await = Some(Running { shutdown: tx, join, sweeper, mdns });
        self.set_status(|s| *s = ServerStatus { running: true, ip: ip.to_string(), port: local.port(), discovery, error: None });
        tracing::info!(%local, discovery, "LAN server listening");
        Ok(self.status())
    }

    pub async fn stop(&self) {
        if let Some(r) = self.running.lock().await.take() {
            r.sweeper.abort();
            let _ = r.shutdown.send(());
            // Open WebSockets would block a graceful shutdown forever; give it a moment, then abort.
            let abort = r.join.abort_handle();
            if tokio::time::timeout(std::time::Duration::from_secs(2), r.join).await.is_err() {
                abort.abort();
            }
            drop(r.mdns);
            self.set_status(|s| {
                s.running = false;
                s.discovery = false;
            });
            tracing::info!("LAN server stopped");
        }
    }

    fn set_status(&self, f: impl FnOnce(&mut ServerStatus)) {
        f(&mut self.status.write().unwrap_or_else(|e| e.into_inner()));
    }
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/api/health", get(health))
        .route("/api/sessions/lookup", post(lookup))
        .route("/ws", get(ws_upgrade))
        .layer(DefaultBodyLimit::max(config::MAX_HTTP_BODY_BYTES))
        .layer(RequestBodyLimitLayer::new(config::MAX_HTTP_BODY_BYTES))
        .with_state(state)
}

async fn health() -> Json<serde_json::Value> {
    Json(json!({ "app": config::APP_NAME, "version": env!("CARGO_PKG_VERSION"), "ok": true }))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LookupRequest {
    session_code: String,
}

/// Lets the student client confirm a code before opening a WebSocket. Reveals only the exam
/// title and status of an open session, and counts bad guesses per address.
async fn lookup(State(st): State<AppState>, ConnectInfo(addr): ConnectInfo<SocketAddr>, Json(req): Json<LookupRequest>) -> Response {
    let ip = addr.ip();
    if st.hub.is_blocked(ip) {
        return (StatusCode::TOO_MANY_REQUESTS, Json(json!({ "code": "too_many_attempts", "message": "Too many wrong codes. Wait a minute and try again." }))).into_response();
    }
    match st.db.find_open_session_by_code(&req.session_code).await {
        Ok(Some(s)) => match st.db.get_session_row(&s.id).await {
            Ok(row) => Json(json!({ "sessionId": s.id, "examTitle": row.exam_title, "status": s.status })).into_response(),
            Err(_) => internal(),
        },
        Ok(None) => {
            st.hub.note_failure(ip);
            (StatusCode::NOT_FOUND, Json(json!({ "code": "session_not_found", "message": "No open session has that code." }))).into_response()
        }
        Err(_) => internal(),
    }
}

fn internal() -> Response {
    (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "code": "internal", "message": "The server hit an error." }))).into_response()
}

async fn ws_upgrade(State(st): State<AppState>, ConnectInfo(addr): ConnectInfo<SocketAddr>, ws: WebSocketUpgrade) -> Response {
    ws.max_message_size(config::MAX_WS_MESSAGE_BYTES)
        .max_frame_size(config::MAX_WS_MESSAGE_BYTES)
        .on_upgrade(move |socket| handler::handle_socket(socket, st, addr.ip()))
}
