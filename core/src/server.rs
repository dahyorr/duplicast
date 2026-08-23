use tokio::net::{TcpListener, TcpStream};
use tracing::{info, error, debug, instrument};

use crate::handshake::perform_rtmp_handshake;
use crate::session::run_rtmp_session;
use crate::state::AppState;

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

    // Step 2: Run the RTMP session for the connection's entire life - this
    // negotiates connect/publish, sets up the GStreamer pipeline once a
    // publish is accepted, and streams FLV-wrapped media into it until the
    // client disconnects or finishes publishing.
    run_rtmp_session(&mut socket, remaining_bytes, &peer_addr, state).await
}
