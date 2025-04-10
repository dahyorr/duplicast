use super::utils::{
    flv_header, is_audio_aac_sequence_header, is_video_keyframe_avc_sequence_header,
};

use crate::config::{self, clear_folder};
use crate::events::AppEvents;
use std::{process::Stdio, sync::Arc};
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::Command;

pub async fn start_encoder(
    // initial_data: Vec<u8>,
    app: &AppHandle,
) -> Result<(), Box<dyn std::error::Error>> {
    let log_dir = config::log_output_dir(app);
    let log_file = std::fs::File::create(&log_dir.join("ffmpeg_encoder.log"))?;
    let log_file = Stdio::from(log_file);
    let out_dir = config::hls_output_dir(app);
    clear_folder(&out_dir).unwrap_or_default();
    let out_path = config::hls_playlist_path(app).to_string_lossy().to_string();
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
        "[f=hls:hls_time=6:hls_list_size=8:hls_flags=delete_segments]\"{}\"|[f=flv]pipe:1",
        out_path
    );
    args.extend([
        "-f",
        "hls",
        "-hls_flags",
        "delete_segments",
        "-hls_time",
        "6",
        "-hls_list_size",
        "8",
        "-hls_flags",
        "delete_segments",
        out_path.as_str(),
    ]);
    // args.extend(["-f", "tee", output.as_str()]);
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
    let encoder_tx = state.encoder_tx.clone();

    // possibly store fanout task
    let cloned_state = Arc::clone(&state);

    tokio::spawn(async move {
        let mut buf = [0u8; 4096];
        let mut stdout = stdout;

        loop {
            match stdout.read(&mut buf).await {
                Ok(0) => break,
                Ok(n) => {
                    let chunk = buf[..n].to_vec();

                    // Send to broadcast channel
                    let _ = encoder_tx.send(chunk.clone());

                    // Optionally detect and store headers
                    if is_video_keyframe_avc_sequence_header(&chunk)
                        || is_audio_aac_sequence_header(&chunk)
                    {
                        let mut headers = cloned_state.encoder_sequence_headers.lock().await;
                        headers.push(chunk);
                    }
                }
                Err(e) => {
                    eprintln!("❌ Fanout read error: {}", e);
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
}
