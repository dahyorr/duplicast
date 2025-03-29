use crate::config::{self};
use crate::events::AppEvents;
use std::{fs, process::Stdio, sync::Arc};
use tauri::{AppHandle, Emitter, Manager};
use tokio::process::Command;

pub async fn start_encoder(
    // initial_data: Vec<u8>,
    app: &AppHandle,
) -> Result<(), Box<dyn std::error::Error>> {
    let log_dir = config::log_output_dir();
    std::fs::create_dir_all(&log_dir);
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
            "-c:v",
            "libx264",
            "-c:a",
            "aac",
            "-f",
            "tee"
          // "hls",
          // "-hls_time",
          // "6",
          // "-hls_list_size",
          // "8",
          // "-hls_flags",
          // "delete_segments",
          &output,
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(log_file)
        .spawn()?;

    let mut stdin = ffmpeg.stdin.take().unwrap();
    let stdout = ffmpeg.stdout.take().unwrap();

    let state = app.state::<Arc<config::AppState>>();
    *state.encoder_stdin.lock().await = Some(stdin);
    *state.encoder_stdout.lock().await = Some(stdout);
    *state.encoder_process.lock().await = Some(ffmpeg);
    Ok(())
}

pub async fn stop_encoder(app: &AppHandle) {
    let state = app.state::<Arc<config::AppState>>();
    let mut process_guard = state.encoder_process.lock().await;
    *state.encoder_stdin.lock().await = None;

    if let Some(mut child) = process_guard.take() {
        if let Err(e) = child.kill().await {
            eprintln!("⚠️ Failed to kill encoder process: {}", e);
        } else {
            println!("🛑 Encoder process stopped");
        }
    }
    *state.encoder_stdout.lock().await = None;

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
