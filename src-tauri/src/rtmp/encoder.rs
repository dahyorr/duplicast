use super::{fanout::start_fanout, utils::flv_header};

use crate::config::{self};
use crate::events::AppEvents;
use std::{fs, process::Stdio, sync::Arc};
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

pub async fn start_encoder(
    // initial_data: Vec<u8>,
    app: &AppHandle,
) -> Result<(), Box<dyn std::error::Error>> {
    let log_dir = config::log_output_dir(app);
    let log_file = std::fs::File::create(&log_dir.join("ffmpeg_encoder.log"))?;
    let log_file = Stdio::from(log_file);
    let out_dir = config::hls_output_dir(app);
    let out_path = config::hls_playlist_path(app);
    fs::create_dir_all(out_dir)?;
    let state = app.state::<Arc<config::AppState>>();
    let settings = state.encoder_settings.lock().await.clone();
    let mut args = vec!["-f", "flv", "-i", "pipe:0"];
    let video_bitrate = format!("{}k", settings.video_bitrate);
    let audio_bitrate = format!("{}k", settings.audio_bitrate);
    let bufsize = format!("{}k", settings.bufsize.unwrap_or(8000));
    if settings.use_passthrough {
        args.extend(["-c:v", "copy", "-c:a", "copy"]);
    } else {
        args.extend([
            "-map",
            "0:v",
            "-map",
            "0:a",
            "-c:v",
            &settings.video_codec,
            "-b:v",
            &video_bitrate,
            "-bufsize",
            &bufsize,
            "-preset",
            &settings.preset,
            "-c:a",
            &settings.audio_codec,
            "-b:a",
            &audio_bitrate,
        ]);
        if let Some(tune) = &settings.tune {
            args.extend(["-tune", tune]);
        }
    }
    let output = format!(
        "[f=hls:hls_time=6:hls_list_size=8:hls_flags=delete_segments]{}|[f=flv]pipe:1",
        out_path.to_string_lossy()
    );
    args.extend(["-f", "tee", output.as_str()]);
    let mut ffmpeg = Command::new("ffmpeg")
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(log_file)
        .spawn()?;

    let mut stdin = ffmpeg.stdin.take().unwrap();
    let stdout = ffmpeg.stdout.take().unwrap();

    if stdin.write_all(&flv_header()).await.is_ok() {
        *state.encoder_stdin.lock().await = Some(stdin);
    }
    *state.encoder_process.lock().await = Some(ffmpeg);

    // possibly store fanout task
    let app_clone = app.clone();

    let _fanout_task = tokio::spawn(async move {
        start_fanout(app_clone, stdout).await;
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
    let out_dir = config::hls_output_dir(&app);
    if out_dir.exists() {
        fs::remove_dir_all(out_dir).unwrap_or_else(|_| {
            eprintln!("⚠️ Failed to remove preview output directory");
        });
    }
}
