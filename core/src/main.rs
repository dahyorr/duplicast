use anyhow::Context;
use gstreamer::glib::object::Cast;
use gstreamer::prelude::{ElementExt, GstBinExt};
use rml_rtmp::handshake::{Handshake, HandshakeProcessResult, PeerType};
use rml_rtmp::sessions::{
    ServerSession, ServerSessionConfig, ServerSessionEvent, ServerSessionResult,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize GStreamer
    gstreamer::init()?;

    // Server configuration
    let port = 1935;
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

async fn handle_rtmp_session(
    socket: &mut TcpStream,
    initial_bytes: Vec<u8>,
    peer_addr: &str,
) -> Result<Vec<u8>> {
    println!("📡 [{}] Handling RTMP session messages...", peer_addr);

    let config = ServerSessionConfig::new();
    let (mut session, initial_results) = ServerSession::new(config)?;
    let mut buffer = initial_bytes;
    let mut read_buffer = vec![0u8; 8192];

    // Process initial results from session creation
    for result in initial_results {
        if let ServerSessionResult::OutboundResponse(packet) = result {
            socket.write_all(&packet.bytes).await?;
            socket.flush().await?;
        }
    }

    loop {
        // Process buffered data
        let results = session.handle_input(&buffer)?;
        buffer.clear();

        for result in results {
            match result {
                ServerSessionResult::OutboundResponse(packet) => {
                    // Send response to client
                    socket.write_all(&packet.bytes).await?;
                    socket.flush().await?;
                }
                ServerSessionResult::RaisedEvent(event) => {
                    match event {
                        ServerSessionEvent::ConnectionRequested {
                            request_id,
                            app_name,
                        } => {
                            println!(
                                "   [{}] Connection requested to app: {}",
                                peer_addr, app_name
                            );
                            // Accept any connection request
                            let responses = session.accept_request(request_id)?;
                            for response in responses {
                                if let ServerSessionResult::OutboundResponse(packet) = response {
                                    socket.write_all(&packet.bytes).await?;
                                    socket.flush().await?;
                                }
                            }
                        }
                        ServerSessionEvent::ReleaseStreamRequested {
                            request_id,
                            app_name,
                            stream_key: key,
                        } => {
                            println!("   [{}] Release stream: {}/{}", peer_addr, app_name, key);
                            let responses = session.accept_request(request_id)?;
                            for response in responses {
                                if let ServerSessionResult::OutboundResponse(packet) = response {
                                    socket.write_all(&packet.bytes).await?;
                                    socket.flush().await?;
                                }
                            }
                        }
                        ServerSessionEvent::PublishStreamRequested {
                            request_id,
                            app_name,
                            stream_key: key,
                            mode,
                        } => {
                            println!(
                                "   [{}] Publish requested: {}/{} (mode: {:?})",
                                peer_addr, app_name, key, mode
                            );

                            // Accept the publish request
                            let responses = session.accept_request(request_id)?;
                            for response in responses {
                                if let ServerSessionResult::OutboundResponse(packet) = response {
                                    socket.write_all(&packet.bytes).await?;
                                    socket.flush().await?;
                                }
                            }

                            println!("✅ [{}] Stream key accepted: {}", peer_addr, key);
                            println!("🎬 [{}] Starting media transmission...", peer_addr);

                            // Return empty buffer - media will come in next reads
                            return Ok(Vec::new());
                        }
                        ServerSessionEvent::StreamMetadataChanged {
                            app_name,
                            stream_key,
                            metadata,
                        } => {
                            println!(
                                "   [{}] Metadata for {}/{}: {:?}",
                                peer_addr, app_name, stream_key, metadata
                            );
                        }
                        ServerSessionEvent::VideoDataReceived {
                            app_name: _,
                            stream_key: _,
                            data,
                            timestamp: _,
                        } => {
                            // Video data received - return it to be processed by GStreamer
                            println!("📹 [{}] First video packet received", peer_addr);
                            return Ok(data.to_vec());
                        }
                        ServerSessionEvent::AudioDataReceived {
                            app_name: _,
                            stream_key: _,
                            data,
                            timestamp: _,
                        } => {
                            // Audio data received - return it to be processed by GStreamer
                            println!("🔊 [{}] First audio packet received", peer_addr);
                            return Ok(data.to_vec());
                        }
                        _ => {
                            println!("   [{}] Other event: {:?}", peer_addr, event);
                        }
                    }
                }
                ServerSessionResult::UnhandleableMessageReceived(_payload) => {
                    // Ignore unhandleable messages
                }
            }
        }

        // Read more data from socket
        let bytes_read = socket.read(&mut read_buffer).await?;
        if bytes_read == 0 {
            return Err(anyhow::anyhow!("Connection closed during RTMP session setup").into());
        }

        buffer.extend_from_slice(&read_buffer[..bytes_read]);
    }
}

async fn perform_rtmp_handshake(socket: &mut TcpStream, peer_addr: &str) -> Result<Vec<u8>> {
    println!("🤝 [{}] Starting RTMP handshake...", peer_addr);

    let mut handshake = Handshake::new(PeerType::Server);
    let mut buffer = [0u8; 4096];
    let mut handshake_step = 1;

    loop {
        // Read data from the client
        let bytes_read = socket.read(&mut buffer).await?;

        if bytes_read == 0 {
            return Err(anyhow::anyhow!("Connection closed during handshake").into());
        }

        println!(
            "   [{}] Step {}: Received {} bytes",
            peer_addr, handshake_step, bytes_read
        );

        // Process the handshake bytes
        match handshake.process_bytes(&buffer[..bytes_read]) {
            Ok(HandshakeProcessResult::InProgress { response_bytes }) => {
                // Handshake in progress - send response
                if !response_bytes.is_empty() {
                    socket.write_all(&response_bytes).await?;
                    socket.flush().await?;
                    println!(
                        "   [{}] Step {}: Sent {} bytes response",
                        peer_addr,
                        handshake_step,
                        response_bytes.len()
                    );
                }
                handshake_step += 1;
            }
            Ok(HandshakeProcessResult::Completed {
                response_bytes,
                remaining_bytes,
            }) => {
                // Handshake complete - send final response
                if !response_bytes.is_empty() {
                    socket.write_all(&response_bytes).await?;
                    socket.flush().await?;
                    println!(
                        "   [{}] Final: Sent {} bytes response",
                        peer_addr,
                        response_bytes.len()
                    );
                }

                println!(
                    "✅ [{}] RTMP handshake completed successfully! ({} bytes remaining)",
                    peer_addr,
                    remaining_bytes.len()
                );

                return Ok(remaining_bytes);
            }
            Err(e) => {
                return Err(anyhow::anyhow!("Handshake failed: {:?}", e).into());
            }
        }
    }
}

fn setup_gstreamer_pipeline(
    peer_addr: &str,
) -> Result<(gstreamer::Pipeline, gstreamer_app::AppSrc)> {
    println!("🎬 [{}] Setting up GStreamer pipeline...", peer_addr);

    // Create the pipeline string
    let pipeline_str = "appsrc name=ingest is-live=true format=time ! \
        flvdemux name=demux \
        demux.video ! h264parse ! queue ! tee name=videotee ! fakesink \
        demux.audio ! aacparse ! queue ! fakesink";

    // Parse and create the pipeline
    let pipeline = gstreamer::parse::launch(pipeline_str)?
        .downcast::<gstreamer::Pipeline>()
        .map_err(|_| anyhow::anyhow!("Failed to cast to Pipeline"))?;

    // Get the appsrc element
    let appsrc = pipeline
        .by_name("ingest")
        .context("Could not find 'ingest' element")?
        .downcast::<gstreamer_app::AppSrc>()
        .map_err(|_| anyhow::anyhow!("'ingest' is not an AppSrc"))?;

    // Start the pipeline
    pipeline.set_state(gstreamer::State::Playing)?;

    println!("✅ [{}] Pipeline ready and playing", peer_addr);

    Ok((pipeline, appsrc))
}

async fn process_rtmp_stream(
    socket: &mut TcpStream,
    appsrc: &gstreamer_app::AppSrc,
    initial_data: Vec<u8>,
    peer_addr: &str,
) -> Result<()> {
    let mut total_bytes = 0;

    // Push any remaining bytes from the handshake first
    if !initial_data.is_empty() {
        let initial_len = initial_data.len();
        println!(
            "📦 [{}] Pushing {} bytes of initial RTMP data",
            peer_addr, initial_len
        );

        let buffer = gstreamer::Buffer::from_slice(initial_data);
        if appsrc.push_buffer(buffer).is_err() {
            return Err(anyhow::anyhow!("Failed to push initial data to pipeline").into());
        }
        total_bytes += initial_len;
    }

    // Main streaming loop
    let mut read_buffer = [0u8; 8192];
    let mut packets_received = 0;

    loop {
        let bytes_read = socket.read(&mut read_buffer).await?;

        if bytes_read == 0 {
            println!(
                "📊 [{}] Stream ended. Total: {} bytes, {} packets",
                peer_addr, total_bytes, packets_received
            );
            break;
        }

        // Push data to GStreamer pipeline
        let buffer = gstreamer::Buffer::from_slice(read_buffer[..bytes_read].to_vec());
        if appsrc.push_buffer(buffer).is_err() {
            println!("⚠️ [{}] Pipeline stopped accepting data", peer_addr);
            break;
        }

        total_bytes += bytes_read;
        packets_received += 1;

        // Log progress every 100 packets
        if packets_received % 100 == 0 {
            println!(
                "📈 [{}] Received {} packets ({} bytes)",
                peer_addr, packets_received, total_bytes
            );
        }
    }

    Ok(())
}
