use crate::config::{self};
use crate::db::{self};
use crate::events::AppEvents;
use byteorder::{BigEndian, WriteBytesExt};
use rml_rtmp::{
    handshake::{Handshake, HandshakeProcessResult, PeerType},
    sessions::{ServerSession, ServerSessionConfig, ServerSessionEvent, ServerSessionResult},
};
use tokio::fs::OpenOptions;
use std::{
    fs,
    process::Stdio,
    sync::{atomic::Ordering, Arc},
};
use tauri::{async_runtime, AppHandle, Emitter, Manager};
use tokio::process::Child;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    process::Command,
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
        if app_state.rtmp_active.swap(true, Ordering::SeqCst) {
            eprintln!("⚠️ Rejecting RTMP connection from {addr}: stream already in use");
            drop(socket); // Close the connection
            continue;
        }
        println!("🔗 Accepted RTMP connection from {addr}");
        let app_clone: AppHandle = app.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_connection(app_clone.clone(), socket).await {
                eprintln!("❌ Error: {}", e);
            }
            let state = app_clone.state::<Arc<config::AppState>>();
            state.rtmp_active.store(false, Ordering::SeqCst);
            println!("📴 RTMP connection ended");
        });
    }
}

async fn handle_connection(
    app: AppHandle,
    mut socket: TcpStream,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("📡 Handling RTMP connection...");

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
                            match handle_session_event(&app, &mut session, event).await {
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
            match start_ffmpeg_preview(&app).await {
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
                            let _ = app_clone.emit(AppEvents::StreamPreviewActive.as_str(), ());
                        } else {
                            eprintln!("⚠️ FFMPEG failed to create hls stream");
                            let _ = app_clone.emit(AppEvents::StreamPreviewFailed.as_str(), ());
                        }
                    });
                    let _ = app.emit(AppEvents::StreamActive.as_str(), ());
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

            let tagged_data = flv_tag(0x08, timestamp.value, &data);

            if let Ok(mut file) = OpenOptions::new()
            .create(true)
            .append(true)
            .open("video_dump.flv")
            .await
        {
            let _ = file.write_all(&tagged_data).await;
        }
    
            let _ = app
                .state::<Arc<config::AppState>>()
                .relay_tx
                .send(tagged_data);

            Ok(vec![])
        }

        ServerSessionEvent::VideoDataReceived {
            data, timestamp, ..
        } => {
            println!("{:?}", data);
            // println!(
            //     "🎥 Video packet @ {} ({} bytes)",
            //     timestamp.value,
            //     data.len()
            // );

            let tagged_data = flv_tag(0x09, timestamp.value, &data);
            let _ = app
                .state::<Arc<config::AppState>>()
                .relay_tx
                .send(tagged_data);

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
            stop_ffmpeg_preview(&app).await;
            app.emit(AppEvents::StreamEnded.as_str(), stream_key)?;
            // Optionally: clean up any associated buffers, files, etc.

            Ok(vec![])
        }

        other => {
            println!("ℹ️  Unhandled RTMP event: {:?}", other);
            Ok(vec![])
        }
    }
}

async fn start_ffmpeg_preview(
    // initial_data: Vec<u8>,
    app: &AppHandle,
) -> Result<(), Box<dyn std::error::Error>> {
    let out_dir = config::hls_output_dir();
    let out_path = config::hls_playlist_path();
    fs::create_dir_all(out_dir)?;
    let mut ffmpeg = Command::new("ffmpeg")
        .args([
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

    let mut stdin = ffmpeg.stdin.take().unwrap();
    let mut rx = app.state::<Arc<config::AppState>>().relay_tx.subscribe();

    let task = tokio::spawn(async move {
        if stdin.write_all(&flv_header()).await.is_ok() {
            while let Ok(tag) = rx.recv().await {
                if stdin.write_all(&tag).await.is_err() {
                    eprintln!("⚠️ Preview FFMPEG exited early");
                    break;
                }
            }
        }
    });

    let state = app.state::<Arc<config::AppState>>();
    *state.preview_task.lock().await = Some(task);

    Ok(())
}

async fn stop_ffmpeg_preview(app: &AppHandle) {
    let state = app.state::<Arc<config::AppState>>();
    let mut task_guard = state.preview_task.lock().await;

    if let Some(handle) = task_guard.take() {
        handle.abort(); // cancels the running task
        println!("🛑 Preview ffmpeg task stopped.");
    }
    app.emit(AppEvents::StreamPreviewEnded.as_str(), ())
        .unwrap_or_else(|_| {
            eprintln!("⚠️ Failed to emit stream preview stopped event");
        });
    let out_dir = config::hls_output_dir();
    if out_dir.exists() {
        fs::remove_dir_all(out_dir).unwrap_or_else(|_| {
            eprintln!("⚠️ Failed to remove preview output directory");
        });
    }
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

pub async fn start_relay(state: &Arc<config::AppState>, relay: &db::RelayTarget) {
    let mut relays = state.relays.lock().await;

    if relays.contains_key(&relay.id) {
        eprintln!("⚠️ Relay  id:{} already exists", relay.id);
        return;
    }
    match spawn_ffmpeg_relay(state, &relay.url, &relay.stream_key).await {
        Ok(child) => {
            relays.insert(relay.id, child);
            println!("🟢 Started relay id:{}", relay.id);
        }
        Err(e) => eprintln!("❌ Failed to start relay id:{}: {}", relay.id, e),
    }
}

pub async fn stop_relay(app: &AppHandle, id: i64) {
    let state = app.state::<Arc<config::AppState>>();
    let mut relays = state.relays.lock().await;
    if let Some(mut child) = relays.remove(&id) {
        if let Err(e) = child.kill().await {
            eprintln!("⚠️ Failed to kill relay process: {}", e);
        } else {
            println!("🛑 Stopped relay id:{}", id);
        }
    } else {
        println!("⚠️ Relay id:{} not found", id);
    }
}

pub async fn start_relays(state: &Arc<config::AppState>) {
    let pool = db::get_db_pool();
    let targets = db::get_active_relay_targets(pool).await.unwrap_or_default();
    print!("{:?}", targets);
    for relay in targets {
        start_relay(state, &relay).await;
    }
}

async fn stop_relays(app: &AppHandle) {
    let state = app.state::<Arc<config::AppState>>();
    let mut relays = state.relays.lock().await;
    for (_, child) in relays.iter_mut() {
        if let Err(e) = child.kill().await {
            eprintln!("⚠️ Failed to kill relay process: {}", e);
        }
    }
    relays.clear();
}

async fn spawn_ffmpeg_relay(
    state: &Arc<config::AppState>,
    target_url: &str,
    stream_key: &str,
) -> Result<Child, Box<dyn std::error::Error>> {
    let mut rx = state.relay_tx.subscribe();
    let mut child = Command::new("ffmpeg")
        .args([
            "-f",
            "flv",
            "-i",
            "pipe:0",
            "-c:v",
            "libx264",
            "-preset",
            "veryfast",
            "-tune",
            "zerolatency",
            "-c:a",
            "aac",
            "-f",
            "flv",
            &format!("{}/{}", target_url, stream_key),
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .spawn()?;

    let mut stdin = child.stdin.take().unwrap();
    let url_clone = target_url.to_string();

    tokio::spawn(async move {
        if stdin.write_all(&flv_header()).await.is_ok() {
            while let Ok(packet) = rx.recv().await {
                if stdin.write_all(&packet).await.is_err() {
                    eprintln!("⚠️ Relay to {} interrupted", url_clone);
                    break;
                }
            }
        }
    });

    Ok(child)
}
