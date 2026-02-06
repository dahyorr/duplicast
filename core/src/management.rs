use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post},
};
use serde_json::json;
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;
use tracing::{info, warn};
use uuid::Uuid;

use crate::state::AppState;
use crate::types::{CreateRelayRequest, StartRelayRequest, StreamInfo, WebRTCAnswer, WebRTCOffer};

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

    info!(port = port, "Management API server starting");
    println!("🌐 Management API started on http://{}", addr);
    println!("   📊 Stats:     GET  http://{}/api/stats", addr);
    println!("   📺 Streams:   GET  http://{}/api/streams", addr);
    println!("   🔄 Relays:    GET  http://{}/api/relays", addr);
    println!(
        "   🎬 WebRTC:    POST http://{}/api/streams/:id/webrtc/offer",
        addr
    );
    println!("================================================");

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
    let active_relays = relays.iter().filter(|r| r.stream_id.is_some()).count();

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
    let relay_id = state.create_relay(req.name, req.target_url).await;

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
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    match state.get_relay(id).await {
        Some(relay) => Ok(Json(relay)),
        None => Err(StatusCode::NOT_FOUND),
    }
}

async fn stop_relay(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, StatusCode> {
    state
        .stop_relay(id)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;

    match state.get_relay(id).await {
        Some(relay) => Ok(Json(relay)),
        None => Err(StatusCode::NOT_FOUND),
    }
}

async fn webrtc_offer(
    State(_state): State<AppState>,
    Path(_id): Path<Uuid>,
    Json(offer): Json<WebRTCOffer>,
) -> Result<impl IntoResponse, StatusCode> {
    // TODO: Implement WebRTC negotiation with GStreamer
    // For now, return a placeholder response
    println!("📹 WebRTC offer received: {:?}", offer.type_);

    let answer = WebRTCAnswer {
        sdp: "v=0\r\no=- 0 0 IN IP4 0.0.0.0\r\ns=-\r\nt=0 0\r\n".to_string(),
        type_: "answer".to_string(),
    };

    Ok(Json(answer))
}
