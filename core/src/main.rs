mod handshake;
mod management;
mod pipeline;
mod server;
mod session;
mod state;
mod stream;
mod types;

use state::AppState;

type Result<T> = anyhow::Result<T>;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize GStreamer
    gstreamer::init()?;

    // Create shared state
    let state = AppState::new();

    // Server configuration
    let rtmp_port = 1935;
    let management_port = 8080;

    // Start management server in background
    let management_state = state.clone();
    tokio::spawn(async move {
        if let Err(e) = management::start_management_server(management_state, management_port).await {
            eprintln!("Management server error: {}", e);
        }
    });

    // Start the RTMP server (blocking)
    server::start_server(rtmp_port, state).await?;

    Ok(())
}
