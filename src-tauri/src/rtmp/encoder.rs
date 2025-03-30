use super::utils::flv_header;

use crate::config::{self};
use crate::events::AppEvents;
use crate::rtmp::utils::{is_audio_aac_sequence_header, is_video_keyframe_avc_sequence_header};
use std::{fs, process::Stdio, sync::Arc};
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::Command;

pub async fn start_encoder(
    // initial_data: Vec<u8>,
    app: &AppHandle,
) -> Result<(), Box<dyn std::error::Error>> {
    let log_dir = config::log_output_dir();
    let log_file = std::fs::File::create(&log_dir.join("ffmpeg_encoder.log"))?;
    let log_file = Stdio::from(log_file);

    let out_dir = config::hls_output_dir();
    let out_path = config::hls_playlist_path();
    fs::create_dir_all(out_dir)?;
    let output = format!(
        "[f=hls:hls_time=6:hls_list_size=8:hls_flags=delete_segments]{}|[f=flv]pipe:1",
        out_path.to_string_lossy()
    );
    let mut ffmpeg = Command::new("ffmpeg")
        .args([
            "-f",
            "flv",
            "-i",
            "pipe:0",
            "-map",
            "0:v",
            "-map",
            "0:a",
            "-c:v",
            "libx264",
            "-preset",
            "veryfast",
            "-tune",
            "zerolatency",
            "-c:a",
            "aac",
            "-f",
            "tee",
            // "hls",
            // "-hls_time",
            // "6",
            // "-hls_list_size",
            // "8",
            // "-hls_flags",
            // "delete_segments",
            output.as_str(),
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(log_file)
        .spawn()?;

    let mut stdin = ffmpeg.stdin.take().unwrap();
    let mut stdout = ffmpeg.stdout.take().unwrap();
    let state = app.state::<Arc<config::AppState>>();

    if stdin.write_all(&flv_header()).await.is_ok() {
        *state.encoder_stdin.lock().await = Some(stdin);
    }
    *state.encoder_process.lock().await = Some(ffmpeg);

    let tx = state.encoder_tx.clone();
    let cloned_state = Arc::clone(&state);     
    // possibly store fanout task
    
    tokio::spawn(async move {
        let mut buf = [0u8; 4096];
        loop {
            match stdout.read(&mut buf).await {
                Ok(0) => break,
                Ok(n) => {
                    let data = &buf[..n];
                    if is_video_keyframe_avc_sequence_header(data)
                        || is_audio_aac_sequence_header(data)
                    {
                        println!("✅ Encoder sequence header received");
                        let mut headers = cloned_state.encoder_sequence_headers.lock().await;
                        headers.push(data.to_vec());
                    }

                    let _ = tx.send(buf[..n].to_vec()); // ignore lag errors
                }
                Err(e) => {
                    eprintln!("❌ Encoder stdout read error: {}", e);
                    break;
                }
            }
        }
    });
    Ok(())
}

pub async fn stop_encoder(app: &AppHandle) {
    let state = app.state::<Arc<config::AppState>>();
    let mut process_guard = state.encoder_process.lock().await;
    *state.encoder_stdin.lock().await = None;

    if let Some(mut child) = process_guard.take() {
        match child.wait().await {
            Ok(status) => {
                println!("🛑 Encoder process exited with status: {}", status);
            }
            Err(e) => {
                eprintln!("⚠️ Failed to wait on encoder process: {}", e);
            }
        }
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
