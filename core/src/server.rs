use tokio::net::{TcpListener, TcpStream};

use crate::handshake::perform_rtmp_handshake;
use crate::pipeline::setup_gstreamer_pipeline;
use crate::session::handle_rtmp_session;
use crate::stream::process_rtmp_stream;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

pub async fn start_server(port: u16) -> Result<()> {
    let addr = format!("0.0.0.0:{}", port);

    // Start the RTMP server
    let listener = TcpListener::bind(&addr).await?;
    println!("🚀 RTMP Server started on {}", addr);
    println!(
        "📡 Listening for connections at rtmp://localhost:{}/live",
        port
    );
    println!("================================================");

    // Accept and handle incoming connections
    loop {
        match listener.accept().await {
            Ok((socket, peer_addr)) => {
                println!("\n🔌 New connection from {}", peer_addr);

                // Spawn a new task for each connection
                tokio::spawn(async move {
                    if let Err(e) = handle_connection(socket, peer_addr.to_string()).await {
                        eprintln!("❌ Error handling connection from {}: {}", peer_addr, e);
                    } else {
                        println!("✅ Connection from {} closed gracefully", peer_addr);
                    }
                });
            }
            Err(e) => {
                eprintln!("❌ Failed to accept connection: {}", e);
            }
        }
    }
}

async fn handle_connection(mut socket: TcpStream, peer_addr: String) -> Result<()> {
    println!("📋 [{}] Starting connection handler", peer_addr);

    // Step 1: Perform RTMP handshake
    let remaining_bytes = perform_rtmp_handshake(&mut socket, &peer_addr).await?;

    // Step 2: Handle RTMP session and get media data
    let media_data = handle_rtmp_session(&mut socket, remaining_bytes, &peer_addr).await?;

    // Step 3: Setup GStreamer pipeline
    let (_pipeline, appsrc) = setup_gstreamer_pipeline(&peer_addr)?;

    println!(
        "🎥 [{}] Stream started, processing media data...",
        peer_addr
    );

    // Step 4: Process the media stream
    process_rtmp_stream(&mut socket, &appsrc, media_data, &peer_addr).await?;

    Ok(())
}
