use axum::{
    Json, Router,
    body::Body,
    extract::{
        Path, Request, State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    http::{HeaderMap, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{delete, get, post},
};
use serde_json::json;
use std::sync::{Arc, Mutex};
use tokio_stream::wrappers::ReceiverStream;
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;
use tracing::{info, warn};
use uuid::Uuid;

use crate::state::AppState;
use crate::types::{
    Config, CreateRelayRequest, IceCandidate, LogEntry, LogLevel, StartRelayRequest, StreamInfo,
    WebRTCAnswer, WebRTCOffer,
};

pub fn create_router(state: AppState) -> Router {
    if std::env::var("DUPLICAST_API_TOKEN").is_err() {
        warn!("DUPLICAST_API_TOKEN not set - management API mutations are unauthenticated");
    }

    // Mutating endpoints - require a bearer token if DUPLICAST_API_TOKEN is set.
    let protected = Router::new()
        .route("/api/relays", post(create_relay))
        .route("/api/relays/{id}", delete(delete_relay))
        .route("/api/relays/{id}/start", post(start_relay))
        .route("/api/relays/{id}/stop", post(stop_relay))
        .route("/api/streams/{id}/webrtc/{session_id}", delete(webrtc_hangup))
        .route("/api/config", axum::routing::put(update_config))
        .route_layer(middleware::from_fn(require_auth));

    // Read-only / viewer endpoints - no auth required.
    let public = Router::new()
        .route("/api/streams", get(list_streams))
        .route("/api/streams/{id}", get(get_stream))
        .route("/api/streams/{id}/info", get(get_stream_info))
        .route("/api/relays", get(list_relays))
        .route("/api/relays/{id}", get(get_relay))
        .route("/api/streams/{id}/webrtc/offer", post(webrtc_offer))
        .route("/api/streams/{id}/webrtc/ice", post(webrtc_ice))
        .route("/api/streams/{id}/flv", get(stream_flv))
        .route("/api/logs", get(get_logs))
        .route("/api/config", get(get_config))
        .route("/api/health", get(health_check))
        .route("/api/stats", get(get_stats))
        .route("/api/ws", get(websocket_handler));

    // Permissive CORS is only needed for `npm run dev` (Vite on a different port than
    // the API). In release builds the frontend is served same-origin via the ServeDir
    // fallback below, so cross-origin requests aren't expected and don't need to be allowed.
    let cors = if cfg!(debug_assertions) {
        CorsLayer::permissive()
    } else {
        CorsLayer::new()
    };

    let api_router = protected.merge(public).with_state(state).layer(cors);

    // Check if the client dist folder exists (override via DUPLICAST_STATIC_DIR,
    // otherwise check next to the executable - matching the layout scripts/build.sh
    // produces - and fall back to the path used by `cargo run` from core/).
    let client_dist = resolve_static_dir();
    if std::path::Path::new(&client_dist).exists() {
        info!(dir = %client_dist, "Serving frontend");
        // Serve static files from client/dist with API fallback
        api_router.fallback_service(ServeDir::new(client_dist))
    } else {
        warn!(dir = %client_dist, "Client dist folder not found. Serving API only.");
        warn!("Run 'cd client && npm run build' to build the frontend, or set DUPLICAST_STATIC_DIR.");
        api_router
    }
}

fn resolve_static_dir() -> String {
    if let Ok(dir) = std::env::var("DUPLICAST_STATIC_DIR") {
        return dir;
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let candidate = dir.join("dist");
            if candidate.exists() {
                return candidate.to_string_lossy().to_string();
            }
        }
    }
    "../client/dist".to_string()
}

async fn require_auth(headers: HeaderMap, request: Request, next: Next) -> Result<Response, StatusCode> {
    if let Ok(token) = std::env::var("DUPLICAST_API_TOKEN") {
        if !token.is_empty() {
            let provided = headers
                .get(header::AUTHORIZATION)
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.strip_prefix("Bearer "));
            if provided != Some(token.as_str()) {
                return Err(StatusCode::UNAUTHORIZED);
            }
        }
    }
    Ok(next.run(request).await)
}

pub async fn start_management_server(state: AppState, port: u16) -> anyhow::Result<()> {
    let app = create_router(state);
    let addr = format!("0.0.0.0:{}", port);

    info!(port = port, addr = %addr, "Management API server starting");

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

// Handler functions

async fn health_check() -> impl IntoResponse {
    Json(json!({
        "status": "healthy",
        "service": "duplicast-core"
    }))
}

fn compute_stats(streams: &[crate::types::Stream], relays: &[crate::types::Relay]) -> serde_json::Value {
    let active_streams = streams.len();
    let active_relays = relays
        .iter()
        .filter(|r| matches!(r.status, crate::types::RelayStatus::Active))
        .count();

    let total_bitrate: u64 = streams.iter().map(|s| s.bitrate.total_bitrate).sum();
    let total_bytes: u64 = streams.iter().map(|s| s.bitrate.total_bytes).sum();

    json!({
        "active_streams": active_streams,
        "total_relays": relays.len(),
        "active_relays": active_relays,
        "total_bitrate": total_bitrate,
        "total_bitrate_mbps": total_bitrate as f64 / 1_000_000.0,
        "total_bytes": total_bytes,
    })
}

async fn get_stats(State(state): State<AppState>) -> impl IntoResponse {
    let streams = state.get_all_streams().await;
    let relays = state.get_all_relays().await;
    Json(compute_stats(&streams, &relays))
}

async fn list_streams(State(state): State<AppState>) -> impl IntoResponse {
    let streams = state.get_all_streams().await;
    Json(streams)
}

async fn get_stream(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, StatusCode> {
    match state.get_stream(id).await {
        Some(stream) => Ok(Json(stream)),
        None => Err(StatusCode::NOT_FOUND),
    }
}

async fn get_stream_info(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, StatusCode> {
    match state.get_stream(id).await {
        Some(stream) => {
            let relays = state.get_relays_for_stream(id).await;
            Ok(Json(StreamInfo { stream, relays }))
        }
        None => Err(StatusCode::NOT_FOUND),
    }
}

async fn list_relays(State(state): State<AppState>) -> impl IntoResponse {
    let relays = state.get_all_relays().await;
    Json(relays)
}

async fn get_relay(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, StatusCode> {
    match state.get_relay(id).await {
        Some(relay) => Ok(Json(relay)),
        None => Err(StatusCode::NOT_FOUND),
    }
}

async fn create_relay(
    State(state): State<AppState>,
    Json(req): Json<CreateRelayRequest>,
) -> impl IntoResponse {
    let relay_id = state.create_relay(req.name, req.rtmp_url, req.stream_key).await;

    match state.get_relay(relay_id).await {
        Some(relay) => (StatusCode::CREATED, Json(relay)).into_response(),
        None => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn delete_relay(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, StatusCode> {
    state
        .delete_relay(id)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;

    Ok(StatusCode::NO_CONTENT)
}

async fn start_relay(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(req): Json<StartRelayRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    // Verify stream exists
    if state.get_stream(req.stream_id).await.is_none() {
        return Err(StatusCode::NOT_FOUND);
    }

    state
        .start_relay(id, req.stream_id)
        .await
        .map_err(|e| {
            warn!(relay_id = %id, error = %e, "Failed to start relay");
            StatusCode::BAD_REQUEST
        })?;

    let relay = state.get_relay(id).await.ok_or(StatusCode::NOT_FOUND)?;
    state
        .add_log(LogEntry {
            id: Uuid::new_v4().to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            level: crate::types::LogLevel::Info,
            message: format!("Relay '{}' started → {}", relay.name, relay.rtmp_url),
            source: "relay".to_string(),
        })
        .await;

    Ok(Json(relay))
}

async fn stop_relay(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, StatusCode> {
    state
        .stop_relay(id)
        .await
        .map_err(|e| {
            warn!(relay_id = %id, error = %e, "Failed to stop relay");
            StatusCode::NOT_FOUND
        })?;

    let relay = state.get_relay(id).await.ok_or(StatusCode::NOT_FOUND)?;
    state
        .add_log(LogEntry {
            id: Uuid::new_v4().to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            level: crate::types::LogLevel::Info,
            message: format!("Relay '{}' stopped", relay.name),
            source: "relay".to_string(),
        })
        .await;

    Ok(Json(relay))
}

async fn webrtc_offer(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(offer): Json<WebRTCOffer>,
) -> Result<impl IntoResponse, StatusCode> {
    info!(stream_id = %id, "Received WebRTC offer");

    if state.get_stream(id).await.is_none() {
        warn!(stream_id = %id, "Stream not found for WebRTC offer");
        return Err(StatusCode::NOT_FOUND);
    }

    // Grab pipeline handles (cheap GLib ref clones).
    let (pipeline, videotee, audiotee) = {
        let pipelines = state.pipelines.read().await;
        let sp = pipelines.get(&id).ok_or_else(|| {
            warn!(stream_id = %id, "No pipeline for stream");
            StatusCode::NOT_FOUND
        })?;
        (sp.pipeline.clone(), sp.videotee.clone(), sp.audiotee.clone())
    };

    // Channel: GStreamer ICE thread → this async task.
    let (answer_tx, answer_rx) =
        tokio::sync::oneshot::channel::<anyhow::Result<String>>();
    let answer_tx = Arc::new(Mutex::new(Some(answer_tx)));

    let session_id = Uuid::new_v4().to_string();
    let offer_sdp = offer.sdp.clone();
    let stun_server = state.get_config().await.stun_server;

    let session = {
        let answer_tx = answer_tx.clone();
        tokio::task::spawn_blocking(move || {
            crate::state::attach_webrtc_to_pipeline(
                &pipeline, &videotee, &audiotee,
                id, &offer_sdp, &stun_server, answer_tx,
            )
        })
        .await
        .map_err(|e| {
            warn!(error = ?e, "spawn_blocking panic during WebRTC attach");
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .map_err(|e| {
            warn!(stream_id = %id, error = %e, "WebRTC pipeline attach failed");
            StatusCode::INTERNAL_SERVER_ERROR
        })?
    };

    state.add_webrtc_session(session_id.clone(), session).await;

    // Wait up to 30 s for ICE gathering to complete.
    let answer_sdp = tokio::time::timeout(
        std::time::Duration::from_secs(30),
        answer_rx,
    )
    .await
    .map_err(|_| {
        warn!(stream_id = %id, "WebRTC ICE gathering timed out");
        StatusCode::GATEWAY_TIMEOUT
    })?
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .map_err(|e| {
        warn!(stream_id = %id, error = %e, "WebRTC answer error");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    info!(stream_id = %id, session_id = %session_id, "WebRTC answer sent");
    state
        .add_log(LogEntry {
            id: Uuid::new_v4().to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            level: LogLevel::Info,
            message: format!("WebRTC viewer connected to stream {}", id),
            source: "webrtc".to_string(),
        })
        .await;

    Ok(Json(WebRTCAnswer {
        sdp: answer_sdp,
        type_: "answer".to_string(),
        session_id,
    }))
}

async fn webrtc_hangup(
    State(state): State<AppState>,
    Path((_stream_id, session_id)): Path<(Uuid, String)>,
) -> impl IntoResponse {
    state.remove_webrtc_session(&session_id).await;
    StatusCode::NO_CONTENT
}

async fn webrtc_ice(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(_candidate): Json<IceCandidate>,
) -> Result<impl IntoResponse, StatusCode> {
    // With gather-then-respond signaling the browser's offer already contains all
    // ICE candidates, so trickle candidates are not needed. Accept and ignore.
    if state.get_stream(id).await.is_none() {
        return Err(StatusCode::NOT_FOUND);
    }
    Ok(StatusCode::OK)
}

/// Streams the live stream as an HTTP-FLV byte stream (video/x-flv), playable via
/// flv.js/Media Source Extensions. Simpler and far more robust than the WebRTC
/// preview - no ICE/DTLS/codec negotiation, at the cost of ~1-3s of buffering
/// latency, which is fine for a personal dashboard preview.
async fn stream_flv(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Response, StatusCode> {
    let (header_bytes, video_config, audio_config, mut rx) =
        state.flv_subscribe(id).await.ok_or(StatusCode::NOT_FOUND)?;

    let (tx, out_rx) = tokio::sync::mpsc::channel::<Result<bytes::Bytes, std::io::Error>>(64);

    tokio::spawn(async move {
        if tx.send(Ok(header_bytes)).await.is_err() {
            return;
        }
        if let Some(v) = video_config {
            if tx.send(Ok(v)).await.is_err() {
                return;
            }
        }
        if let Some(a) = audio_config {
            if tx.send(Ok(a)).await.is_err() {
                return;
            }
        }
        loop {
            match rx.recv().await {
                Ok(tag) => {
                    if tx.send(Ok(tag)).await.is_err() {
                        break;
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    });

    let body = Body::from_stream(ReceiverStream::new(out_rx));

    Response::builder()
        .header(header::CONTENT_TYPE, "video/x-flv")
        .header(header::CACHE_CONTROL, "no-cache")
        .body(body)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

/// Single push connection replacing the old per-resource polling
/// (`/api/stats`, `/api/streams`, `/api/relays`, `/api/logs` on a timer). Sends a
/// combined stats/streams/relays snapshot on an interval, plus new log entries as
/// they're created (real push, not polled) via `AppState::log_tx`. The REST
/// endpoints stay in place for the initial page load and as a fallback.
async fn websocket_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> Response {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

fn ws_message<T: serde::Serialize>(msg_type: &str, payload: &T) -> Option<Message> {
    serde_json::to_string(&json!({ "type": msg_type, "payload": payload }))
        .ok()
        .map(|s| Message::Text(s.into()))
}

async fn handle_socket(mut socket: WebSocket, state: AppState) {
    let mut log_rx = state.log_tx.subscribe();

    let initial_logs = state.get_logs(Some(100)).await;
    if let Some(msg) = ws_message("logs_init", &initial_logs) {
        if socket.send(msg).await.is_err() {
            return;
        }
    }

    let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));

    loop {
        tokio::select! {
            _ = interval.tick() => {
                let streams = state.get_all_streams().await;
                let relays = state.get_all_relays().await;
                let snapshot = json!({
                    "stats": compute_stats(&streams, &relays),
                    "streams": streams,
                    "relays": relays,
                });
                match ws_message("snapshot", &snapshot) {
                    Some(msg) => {
                        if socket.send(msg).await.is_err() {
                            break;
                        }
                    }
                    None => continue,
                }
            }
            log = log_rx.recv() => {
                match log {
                    Ok(entry) => {
                        if let Some(msg) = ws_message("log", &entry) {
                            if socket.send(msg).await.is_err() {
                                break;
                            }
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
            incoming = socket.recv() => {
                match incoming {
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Err(_)) => break,
                    _ => {} // ignore pings/pongs/anything the client sends
                }
            }
        }
    }
}

async fn get_logs(State(state): State<AppState>) -> impl IntoResponse {
    let logs = state.get_logs(Some(200)).await;
    Json(logs)
}

async fn get_config(State(state): State<AppState>) -> impl IntoResponse {
    Json(state.get_config().await)
}

async fn update_config(
    State(state): State<AppState>,
    Json(new_config): Json<Config>,
) -> Result<impl IntoResponse, StatusCode> {
    // Basic validation.
    if new_config.rtmp_port == 0 || new_config.api_port == 0 {
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    }

    state.update_config(new_config).await.map_err(|e| {
        warn!(error = %e, "Failed to persist config");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    state
        .add_log(LogEntry {
            id: uuid::Uuid::new_v4().to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            level: LogLevel::Info,
            message: "Configuration updated".to_string(),
            source: "system".to_string(),
        })
        .await;

    Ok(Json(state.get_config().await))
}

