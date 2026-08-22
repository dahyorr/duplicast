mod flv;
mod handshake;
mod management;
mod pipeline;
mod server;
mod session;
mod state;
mod types;

use state::AppState;
use tracing::info;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};
use types::{LogEntry, LogLevel};
use uuid::Uuid;

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

    // Add startup log entries
    state.add_log(LogEntry {
        id: Uuid::new_v4().to_string(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        level: LogLevel::Info,
        message: "Duplicast server starting up".to_string(),
        source: "system".to_string(),
    }).await;

    state.add_log(LogEntry {
        id: Uuid::new_v4().to_string(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        level: LogLevel::Info,
        message: "GStreamer initialized successfully".to_string(),
        source: "gstreamer".to_string(),
    }).await;

    // Server configuration
    let config = state.get_config().await;
    let rtmp_port = config.rtmp_port;
    let management_port = config.api_port;

    // Start management server in background
    let management_state = state.clone();
    tokio::spawn(async move {
        if let Err(e) = management::start_management_server(management_state, management_port).await
        {
            tracing::error!(error = ?e, "Management server error");
        }
    });

    // Run the RTMP server until it errors out or a shutdown signal arrives.
    let rtmp_state = state.clone();
    tokio::select! {
        result = server::start_server(rtmp_port, rtmp_state) => {
            if let Err(e) = result {
                tracing::error!(error = ?e, "RTMP server error");
            }
        }
        _ = shutdown_signal() => {
            info!("Shutdown signal received, tearing down active pipelines");
        }
    }

    state.shutdown().await;

    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}
