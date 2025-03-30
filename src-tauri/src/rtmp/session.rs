use super::{encoder, utils::flv_tag};

use crate::{config, events::AppEvents};
use rml_rtmp::sessions::{
    ServerSession, ServerSessionConfig, ServerSessionEvent, ServerSessionResult,
};
use std::sync::Arc;
use tauri::{async_runtime, AppHandle, Emitter, Manager};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};

pub async fn handle_session(
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
            match encoder::start_encoder(&app).await {
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
            // println!("🎵 Audio data received: {} bytes", data.len());
            let state = app.state::<Arc<config::AppState>>();
            let tagged_data = flv_tag(0x08, timestamp.value, &data);
            let mut guard = state.encoder_stdin.lock().await;
            if let Some(stdin) = guard.as_mut() {
                if let Err(e) = stdin.write_all(&tagged_data).await {
                    eprintln!("❌ Failed to write to encoder stdin: {}", e);
                }
            }
            Ok(vec![])
        }

        ServerSessionEvent::VideoDataReceived {
            data, timestamp, ..
        } => {
            // println!("📹 Video data received: {} bytes", data.len());
            let state = app.state::<Arc<config::AppState>>();
            let tagged_data = flv_tag(0x09, timestamp.value, &data);
            let mut guard = state.encoder_stdin.lock().await;
            if let Some(stdin) = guard.as_mut() {
                if let Err(e) = stdin.write_all(&tagged_data).await {
                    eprintln!("❌ Failed to write to encoder stdin: {}", e);
                }
            }
            Ok(vec![])
        }

        ServerSessionEvent::StreamMetadataChanged {
            metadata,
            stream_key,
            ..
        } => {
            println!("📊 Metadata for stream {}: {:?}", stream_key, metadata);
            // if let Ok(mut file) = OpenOptions::new()
            //     .create(true)
            //     .append(true)
            //     .open("metadata_dump.txt")
            //     .await
            // {
            //     if let Ok(json) = to_string_pretty(&metadata) {
            //         let _ = file.write_all(json.as_bytes()).await;
            //         let _ = file.write_all(b"\n").await;
            //     }
            // }
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
            encoder::stop_encoder(&app).await;
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
