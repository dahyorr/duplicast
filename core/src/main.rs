mod handshake;
mod pipeline;
mod server;
mod session;
mod stream;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize GStreamer
    gstreamer::init()?;

    // Server configuration
    let port = 1935;

    // Start the RTMP server
    server::start_server(port).await?;

    Ok(())
}
