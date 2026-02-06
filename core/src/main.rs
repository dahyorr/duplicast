mod handshake;
mod management;
mod pipeline;
mod server;
mod session;
mod state;
mod stream;
mod types;

use state::AppState;
use tracing::info;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

type Result<T> = anyhow::Result<T>;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "duplicast_core=info,tower_http=debug,axum=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("Starting Duplicast RTMP Server");

    // Initialize GStreamer
    gstreamer::init()?;
    info!("GStreamer initialized successfully");

    // Create shared state
    let state = AppState::new();
    info!("Application state initialized");

    // Server configuration
    let rtmp_port = 1935;
    let management_port = 8080;

    // Start management server in background
    let management_state = state.clone();
    tokio::spawn(async move {
        if let Err(e) = management::start_management_server(management_state, management_port).await
        {
            tracing::error!(error = ?e, "Management server error");
        }
    });

    // Start the RTMP server (blocking)
    server::start_server(rtmp_port, state).await?;

    Ok(())
}
