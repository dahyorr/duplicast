use rml_rtmp::{
    handshake::{Handshake, HandshakeProcessResult, PeerType},
    sessions::{ServerSession, ServerSessionConfig, ServerSessionEvent, ServerSessionResult},
};
use std::{process::Stdio, sync::Arc};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    process::{ChildStdin, Command},
    sync::Mutex,
};

pub async fn init_rtmp_server() {
    let listener = TcpListener::bind("0.0.0.0:1580")
        .await
        .expect("Failed to bind");
    println!("🟢 RTMP server listening on rtmp://localhost:1580");

    loop {
        let (socket, addr) = listener.accept().await.expect("Failed to accept");
        println!("🔗 Connection from {}", addr);
        tokio::spawn(async move {
            if let Err(e) = handle_connection(socket).await {
                eprintln!("❌ Error: {}", e);
            }
        });
    }
}

async fn handle_connection(mut socket: TcpStream) -> Result<(), Box<dyn std::error::Error>> {
    println!("📡 Handling new RTMP connection...");

    let mut handshake = Handshake::new(PeerType::Server);
    let mut buffer = [0u8; 4096];
    let mut received_data = Vec::new();

    loop {
        let n = socket.read(&mut buffer).await?;
        if n == 0 {
            return Err("🔌 Connection closed during handshake".into());
        }
        received_data.extend_from_slice(&buffer[..n]);

        match handshake.process_bytes(&received_data) {
            Ok(HandshakeProcessResult::InProgress { response_bytes }) => {
                socket.write_all(&response_bytes).await?;
                received_data.clear(); // Reset buffer until next chunk
            }

            Ok(HandshakeProcessResult::Completed {
                response_bytes,
                remaining_bytes,
            }) => {
                socket.write_all(&response_bytes).await?;
                println!("✅ RTMP handshake complete 🤝");
                return handle_session(socket, remaining_bytes.to_vec()).await;
            }

            Err(e) => {
                return Err(format!("❌ Handshake error: {:?}", e).into());
            }
        };
    }
}

async fn handle_session(
    mut socket: TcpStream,
    mut received_data: Vec<u8>,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("📦 Starting RTMP session");
    let config = ServerSessionConfig::new();
    let (mut session, initial_session_results) = match ServerSession::new(config) {
        Ok(results) => results,
        Err(error) => return Err(error.to_string().into()),
    };

    let ffmpeg_stdin: Arc<Mutex<Option<ChildStdin>>> = Arc::new(Mutex::new(None));

    for result in initial_session_results {
        if let ServerSessionResult::OutboundResponse(packet) = result {
            socket.write_all(&packet.bytes).await?;
        }
    }

    let mut buffer = [0u8; 4096];

    loop {
        // Read more if we’ve exhausted the buffer

        if received_data.is_empty() {
            let n = socket.read(&mut buffer).await?;
            if n == 0 {
                println!("🔌 Client disconnected.");
                return Ok(());
            }
            received_data.extend_from_slice(&buffer[..n]);
        }

        match session.handle_input(&received_data) {
            Ok(results) => {
                received_data.clear();
                for result in results {
                    match result {
                        ServerSessionResult::OutboundResponse(packet) => {
                            // Write the RTMP chunk to the wire
                            socket.write_all(&packet.bytes).await?;
                        }
                        ServerSessionResult::RaisedEvent(event) => {
                            let stdin_clone = ffmpeg_stdin.clone();
                            match handle_session_event(&mut session, event, stdin_clone).await {
                                Ok(responses) => {
                                    for res in responses {
                                        if let ServerSessionResult::OutboundResponse(packet) = res {
                                            socket.write_all(&packet.bytes).await?;
                                        }
                                    }
                                }
                                Err(e) => {
                                    eprintln!("❌ Failed to handle session event: {}", e);
                                    return Err(e);
                                }
                            }
                        }
                        ServerSessionResult::UnhandleableMessageReceived(msg) => {
                            println!("⚠️  Unhandleable message: {:?}", msg);
                        }
                    }
                }
            }
            Err(e) => {
                return Err(format!("❌ Session error: {:?}", e).into());
            }
        }
    }
}
async fn handle_session_event(
    session: &mut ServerSession,
    event: ServerSessionEvent,
    ffmpeg_stdin: Arc<Mutex<Option<ChildStdin>>>,
) -> Result<Vec<ServerSessionResult>, Box<dyn std::error::Error + Send + Sync>> {
    // // create video file to stream into
    // let video_file = std::fs::File::create("public/preview/video.mp4")?;
    // let mut video_file = tokio::fs::File::from_std(video_file);

    match event {
        ServerSessionEvent::ConnectionRequested {
            request_id,
            app_name,
            ..
        } => {
            println!(
                "🌐 Connection requested for app: {}: {}",
                app_name, request_id
            );
            Ok(session.accept_request(request_id)?)
        }
        ServerSessionEvent::PublishStreamRequested {
            request_id,
            stream_key,
            ..
        } => {
            println!("📡 Publish requested for stream key: {}", stream_key);
            match start_ffmpeg(ffmpeg_stdin).await {
                Ok(_) => {
                    println!("🎥 FFMPEG started");
                    Ok(session.accept_request(request_id)?)
                }
                Err(e) => {
                    eprintln!("❌ Failed to start FFMPEG: {}", e);
                    _ = session.reject_request(request_id, "01", "Failed to start FFMPEG");
                    Ok(vec![])
                }
            }
        }

        ServerSessionEvent::AudioDataReceived {
            data, timestamp, ..
        } => {
            // println!(
            //     "🔊 Audio packet @ {} ({} bytes)",
            //     timestamp.value,
            //     data.len()
            // );
            // let mut stdin_lock = ffmpeg_stdin.lock().await;

            // if let Some(stdin) = stdin_lock.as_mut() {
            //     stdin.write_all(&data).await?; // ✅ fully async
            // }
            Ok(vec![])
        }

        ServerSessionEvent::VideoDataReceived {
            data, timestamp, ..
        } => {
            // println!(
            //     "🎥 Video packet @ {} ({} bytes)",
            //     timestamp.value,
            //     data.len()
            // );
            let mut stdin_lock = ffmpeg_stdin.lock().await;

            if let Some(stdin) = stdin_lock.as_mut() {
                stdin.write_all(&data).await?; // ✅ fully async
            }
            Ok(vec![])
        }

        ServerSessionEvent::StreamMetadataChanged {
            metadata,
            stream_key,
            ..
        } => {
            println!("📊 Metadata for stream {}: {:?}", stream_key, metadata);
            // println!("Metadata: {}", metadata.());
            Ok(vec![])
        }

        ServerSessionEvent::PublishStreamFinished {
            stream_key,
            app_name,
            ..
        } => {
            println!(
                "📴 Publish finished for stream '{}' (id {})",
                app_name, stream_key
            );
            println!("🛑 Stream ended. Closing ffmpeg.");

            let mut stdin_lock = ffmpeg_stdin.lock().await;
            *stdin_lock = None; // Drop the handle so ffmpeg exits

            // Optionally: clean up any associated buffers, files, etc.

            Ok(vec![])
        }

        ServerSessionEvent::PlayStreamRequested {
            request_id,
            stream_key,
            ..
        } => {
            // non applicable
            println!("▶️ Play requested for stream key: {}", stream_key);
            Ok(session.accept_request(request_id)?)
        }

        other => {
            println!("ℹ️  Unhandled RTMP event: {:?}", other);
            Ok(vec![])
        }
    }
}

async fn start_ffmpeg(
    // initial_data: Vec<u8>,
    ffmpeg_stdin: Arc<Mutex<Option<ChildStdin>>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut ffmpeg = Command::new("ffmpeg")
        .args([
            "-f",
            "flv",
            "-i",
            "-",
            "-c:v",
            "libx264",
            "-c:a",
            "aac",
            "-f",
            "hls",
            "-hls_time",
            "2",
            "-hls_list_size",
            "5",
            "-hls_flags",
            "delete_segments",
            "./public/preview/playlist.m3u8",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()?;

    let mut stdin_lock = ffmpeg_stdin.lock().await;
    *stdin_lock = ffmpeg.stdin.take();

    Ok(())
}
