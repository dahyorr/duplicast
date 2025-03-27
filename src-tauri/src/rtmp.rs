use crate::config::{self};
use crate::events::AppEvents;
use byteorder::{BigEndian, WriteBytesExt};
use rml_rtmp::{
    handshake::{Handshake, HandshakeProcessResult, PeerType},
    sessions::{ServerSession, ServerSessionConfig, ServerSessionEvent, ServerSessionResult},
};
use std::{
    fs,
    process::Stdio,
    sync::{atomic::Ordering, Arc},
};
use tauri::{async_runtime, AppHandle, Emitter, Manager};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    process::{ChildStdin, Command},
    sync::Mutex,
};

pub async fn init_rtmp_server(app: AppHandle, port: u16) {
    let listener = TcpListener::bind(format!("0.0.0.0:{}", port))
        .await
        .expect("Failed to bind");
    println!("🟢 RTMP server listening on rtmp://localhost:1580");
    let app_state = app.state::<Arc<crate::config::AppState>>();
    app_state.rtmp_ready.store(true, Ordering::SeqCst);
    loop {
        let (socket, addr) = listener.accept().await.expect("Failed to accept");
        println!("🔗 Connection from {}", addr);
        let app_clone: AppHandle = app.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_connection(app_clone.clone(), socket).await {
                eprintln!("❌ Error: {}", e);
            }
        });
    }
}

async fn handle_connection(
    app: AppHandle,
    mut socket: TcpStream,
) -> Result<(), Box<dyn std::error::Error>> {
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
                return handle_session(&app, socket, remaining_bytes.to_vec()).await;
            }

            Err(e) => {
                return Err(format!("❌ Handshake error: {:?}", e).into());
            }
        };
    }
}

async fn handle_session(
    app: &AppHandle,
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
                            match handle_session_event(&app, &mut session, event, stdin_clone).await
                            {
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
    app: &AppHandle,
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
                    // wait for playlist to be created in new thread
                    let app_clone = app.clone();
                    async_runtime::spawn(async move {
                        let playlist_path = config::hls_playlist_path();
                        use tokio::time::{sleep, Duration};
                        let mut attempts = 0;
                        while !playlist_path.exists() && attempts < 50 {
                            sleep(Duration::from_millis(500)).await;
                            attempts += 1;
                        }
                        if playlist_path.exists() {
                            println!("✅ FFMPEG started successfully");
                            let _ = app_clone.emit(AppEvents::StreamStarted.as_str(), ());
                        } else {
                            eprintln!("⚠️ FFMPEG failed to create hls stream");
                            let _ = app_clone.emit(AppEvents::StreamPreviewFailed.as_str(), ());
                        }
                    });

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
            let mut stdin_lock = ffmpeg_stdin.lock().await;

            if let Some(stdin) = stdin_lock.as_mut() {
                let tag = flv_tag(0x08, timestamp.value, &data);
                stdin.write_all(&tag).await?;
            }
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
                let tag = flv_tag(0x09, timestamp.value, &data);
                stdin.write_all(&tag).await?;
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

            app.emit(AppEvents::StreamStopped.as_str(), stream_key)?;

            let output_dir = config::hls_output_dir();
            if output_dir.exists() {
                fs::remove_dir_all(output_dir)?;
            }
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
    let out_dir = config::hls_output_dir();
    let out_path = config::hls_playlist_path();
    fs::create_dir_all(out_dir)?;
    let mut ffmpeg = Command::new("ffmpeg")
        .args([
            // "-loglevel",
            // "debug",
            "-f",
            "flv",
            "-i",
            "pipe:0",
            "-c:v",
            "libx264",
            "-c:a",
            "aac",
            "-f",
            "hls",
            "-hls_time",
            "6",
            "-hls_list_size",
            "8",
            "-hls_flags",
            "delete_segments",
            &out_path.to_string_lossy().to_string(),
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()?;

    let mut stdin_lock = ffmpeg_stdin.lock().await;
    *stdin_lock = ffmpeg.stdin.take();

    if let Some(stdin) = stdin_lock.as_mut() {
        let header = flv_header();
        stdin.write_all(&header).await?;
    }

    Ok(())
}

fn flv_header() -> Vec<u8> {
    let mut header = Vec::new();

    // Signature: "FLV"
    header.extend_from_slice(b"FLV");

    // Version: 1
    header.push(0x01);

    // Flags: 0x05 = audio + video
    header.push(0x05);

    // DataOffset: header size (9)
    header.extend_from_slice(&[0x00, 0x00, 0x00, 0x09]);

    // PreviousTagSize0: always 0
    header.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]);

    header
}

fn flv_tag(tag_type: u8, timestamp: u32, data: &[u8]) -> Vec<u8> {
    let mut tag = Vec::new();
    let data_size = data.len() as u32;

    // Tag header (11 bytes)
    tag.push(tag_type); // 0x08 = audio, 0x09 = video

    tag.write_u24::<BigEndian>(data_size).unwrap(); // DataSize
    tag.write_u24::<BigEndian>(timestamp & 0xFFFFFF).unwrap(); // Timestamp (lower 24 bits)
    tag.push(((timestamp >> 24) & 0xFF) as u8); // TimestampExtended
    tag.write_u24::<BigEndian>(0).unwrap(); // StreamID (always 0)

    // Payload
    tag.extend_from_slice(data);

    // PreviousTagSize
    let total_size = 11 + data.len();
    byteorder::WriteBytesExt::write_u32::<BigEndian>(&mut tag, total_size as u32).unwrap();
    tag
}
