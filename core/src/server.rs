use tokio::net::{TcpListener, TcpStream};
use tracing::{info, error, debug, instrument};
use uuid::Uuid;

use crate::handshake::perform_rtmp_handshake;
use crate::pipeline::setup_gstreamer_pipeline;
use crate::session::handle_rtmp_session;
use crate::state::AppState;
use crate::stream::process_rtmp_stream;
use crate::types::{LogEntry, LogLevel};

type Result<T> = anyhow::Result<T>;

pub async fn start_server(port: u16, state: AppState) -> Result<()> {
    let addr = format!("0.0.0.0:{}", port);

    // Start the RTMP server
    let listener = TcpListener::bind(&addr).await?;
    info!(port = port, addr = %addr, "RTMP server listening");

    // Accept and handle incoming connections
    loop {
        match listener.accept().await {
            Ok((socket, peer_addr)) => {
                info!(client_addr = %peer_addr, "New RTMP connection");

                let state_clone = state.clone();
                // Spawn a new task for each connection
                tokio::spawn(async move {
                    if let Err(e) = handle_connection(socket, peer_addr.to_string(), state_clone).await {
                        error!(client_addr = %peer_addr, error = ?e, "Connection error");
                    } else {
                        info!(client_addr = %peer_addr, "Connection closed gracefully");
                    }
                });
            }
            Err(e) => {
                error!(error = ?e, "Failed to accept connection");
            }
        }
    }
}

#[instrument(skip(socket, state), fields(client_addr = %peer_addr))]
async fn handle_connection(mut socket: TcpStream, peer_addr: String, state: AppState) -> Result<()> {
    debug!("Starting connection handler");

    // Step 1: Perform RTMP handshake
    let remaining_bytes = perform_rtmp_handshake(&mut socket, &peer_addr).await?;
    info!("RTMP handshake completed");

    // Step 2: Handle RTMP session and get media data, stream info
    let (media_data, stream_key, app_name) = handle_rtmp_session(&mut socket, remaining_bytes, &peer_addr).await?;
    info!(stream_key = %stream_key, app_name = %app_name, "Stream published");

    // Step 3: Register stream with state management
    let stream_id = state.register_stream(stream_key.clone(), app_name, peer_addr.clone()).await;
    info!(stream_id = %stream_id, stream_key = %stream_key, "Stream registered");

    // Log stream connection
    state.add_log(LogEntry {
        id: Uuid::new_v4().to_string(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        level: LogLevel::Info,
        message: format!("Stream '{}' connected from {}", stream_key, peer_addr),
        source: "rtmp".to_string(),
    }).await;

    // Step 4: Setup GStreamer pipeline
    let pipeline_setup = setup_gstreamer_pipeline(&peer_addr)?;
    let appsrc = pipeline_setup.appsrc.clone();
    debug!("GStreamer pipeline created");

    state.register_pipeline(stream_id, pipeline_setup).await;

    info!("Starting media data processing");

    // Step 5: Process the media stream with monitoring
    let result = process_rtmp_stream(&mut socket, &appsrc, media_data, &peer_addr, stream_id, state.clone()).await;

    // Step 6: Unregister stream when done (also destroys pipeline and stops relays)
    state.unregister_stream(stream_id).await;
    info!(stream_id = %stream_id, "Stream unregistered");

    // Log stream disconnection
    state.add_log(LogEntry {
        id: Uuid::new_v4().to_string(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        level: LogLevel::Info,
        message: format!("Stream '{}' disconnected", stream_key),
        source: "rtmp".to_string(),
    }).await;

    result
}
