use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post},
};
use serde_json::json;
use std::sync::{Arc, Mutex};
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
    let api_router = Router::new()
        // Stream endpoints
        .route("/api/streams", get(list_streams))
        .route("/api/streams/{id}", get(get_stream))
        .route("/api/streams/{id}/info", get(get_stream_info))
        // Relay endpoints
        .route("/api/relays", get(list_relays))
        .route("/api/relays", post(create_relay))
        .route("/api/relays/{id}", get(get_relay))
        .route("/api/relays/{id}", delete(delete_relay))
        .route("/api/relays/{id}/start", post(start_relay))
        .route("/api/relays/{id}/stop", post(stop_relay))
        // WebRTC preview endpoints
        .route("/api/streams/{id}/webrtc/offer", post(webrtc_offer))
        .route("/api/streams/{id}/webrtc/ice", post(webrtc_ice))
        // Logs endpoint
        .route("/api/logs", get(get_logs))
        // Config endpoints
        .route("/api/config", get(get_config))
        .route("/api/config", axum::routing::put(update_config))
        // Health check and stats
        .route("/api/health", get(health_check))
        .route("/api/stats", get(get_stats))
        .with_state(state)
        .layer(CorsLayer::permissive());

    // Check if client dist folder exists
    let client_dist = std::path::Path::new("../client/dist");
    if client_dist.exists() {
        info!("Serving frontend from ../client/dist");
        // Serve static files from client/dist with API fallback
        api_router.fallback_service(ServeDir::new("../client/dist"))
    } else {
        warn!("Client dist folder not found. Serving API only.");
        warn!("Run 'cd client && npm run build' to build the frontend.");
        api_router
    }
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

async fn get_stats(State(state): State<AppState>) -> impl IntoResponse {
    let streams = state.get_all_streams().await;
    let relays = state.get_all_relays().await;

    let active_streams = streams.len();
    let active_relays = relays
        .iter()
        .filter(|r| matches!(r.status, crate::types::RelayStatus::Active))
        .count();

    let total_bitrate: u64 = streams.iter().map(|s| s.bitrate.total_bitrate).sum();
    let total_bytes: u64 = streams.iter().map(|s| s.bitrate.total_bytes).sum();

    Json(json!({
        "active_streams": active_streams,
        "total_relays": relays.len(),
        "active_relays": active_relays,
        "total_bitrate": total_bitrate,
        "total_bitrate_mbps": total_bitrate as f64 / 1_000_000.0,
        "total_bytes": total_bytes,
    }))
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

    let session = {
        let answer_tx = answer_tx.clone();
        tokio::task::spawn_blocking(move || {
            crate::state::attach_webrtc_to_pipeline(
                &pipeline, &videotee, &audiotee,
                id, &offer_sdp, answer_tx,
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
    }))
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
    if !["debug", "info", "warn", "error"].contains(&new_config.log_level.as_str()) {
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

