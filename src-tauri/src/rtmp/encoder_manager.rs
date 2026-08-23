use crate::config::{self, clear_folder, AppState};
use crate::events::AppEvents;
use crate::rtmp::utils::flv_header;
use bytes::Bytes;
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::{Child, Command};
use tokio::sync::mpsc;

/// Manages the lifecycle of the FFmpeg encoder process independently of RTMP sessions
pub struct EncoderManager {
    app: AppHandle,
    state: Arc<AppState>,
    write_tx: Option<mpsc::Sender<Bytes>>,
    process: Option<Child>,
    _write_task: Option<tokio::task::JoinHandle<()>>,
    _fanout_task: Option<tokio::task::JoinHandle<()>>,
}

impl EncoderManager {
    pub fn new(app: AppHandle, state: Arc<AppState>) -> Self {
        Self {
            app,
            state,
            write_tx: None,
            process: None,
            _write_task: None,
            _fanout_task: None,
        }
    }

    /// Start the encoder if not already running
    pub async fn start(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.process.is_some() {
            return Ok(()); // Already running
        }

        let log_dir = config::log_output_dir(&self.app);
        let log_file = std::fs::File::create(&log_dir.join("ffmpeg_encoder.log"))?;
        let log_file = std::process::Stdio::from(log_file);

        let out_dir = config::hls_output_dir(&self.app);
        clear_folder(&out_dir).unwrap_or_default();
        let out_path = config::hls_playlist_path(&self.app);

        let settings = self.state.encoder_settings.lock().await;
        let video_bitrate = format!("{}k", settings.video_bitrate);
        let audio_bitrate = format!("{}k", settings.audio_bitrate);
        let bufsize = format!("{}k", settings.bufsize.unwrap_or(8000));

        let mut ffmpeg_cmd = Command::new("ffmpeg");
        ffmpeg_cmd
            .arg("-f")
            .arg("flv")
            .arg("-i")
            .arg("pipe:0")
            .arg("-map")
            .arg("0");

        if settings.use_passthrough {
            ffmpeg_cmd.arg("-c:v").arg("copy").arg("-c:a").arg("copy");
        } else {
            ffmpeg_cmd
                .arg("-c:v")
                .arg(&settings.video_codec)
                .arg("-b:v")
                .arg(&video_bitrate)
                .arg("-bufsize")
                .arg(&bufsize)
                .arg("-preset")
                .arg(&settings.preset)
                .arg("-c:a")
                .arg(&settings.audio_codec)
                .arg("-b:a")
                .arg(&audio_bitrate);
            if let Some(tune) = &settings.tune {
                ffmpeg_cmd.arg("-tune").arg(tune);
            }
        }

        let tee_spec = format!(
            "[f=hls:hls_time=6:hls_list_size=8:hls_flags=delete_segments]{}|[f=flv]pipe:1",
            out_path.display()
        );

        ffmpeg_cmd
            .arg("-f")
            .arg("tee")
            .arg(&tee_spec)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(log_file);

        let mut ffmpeg = ffmpeg_cmd.spawn()?;
        let mut stdin = ffmpeg.stdin.take().unwrap();
        let stdout = ffmpeg.stdout.take().unwrap();

        // Write FLV header
        stdin.write_all(&flv_header()).await?;

        // Create dedicated write task to avoid blocking
        let (write_tx, mut write_rx) = mpsc::channel::<Bytes>(1024);
        let _write_task = tokio::spawn(async move {
            while let Some(data) = write_rx.recv().await {
                if let Err(e) = stdin.write_all(&data).await {
                    eprintln!("❌ Error writing to encoder stdin: {}", e);
                    break;
                }
            }
        });

        self.write_tx = Some(write_tx);

        // Fanout task - read from encoder stdout and broadcast
        let encoder_tx = self.state.encoder_tx.clone();
        let cloned_state = Arc::clone(&self.state);
        let _fanout_task = tokio::spawn(async move {
            let mut buf = vec![0u8; 8192];
            let mut stdout = stdout;

            loop {
                match stdout.read(&mut buf).await {
                    Ok(0) => break,
                    Ok(n) => {
                        let chunk = Bytes::copy_from_slice(&buf[..n]);

                        // Send to broadcast channel (relays will receive)
                        let _ = encoder_tx.send(chunk.clone());

                        // Store sequence headers
                        if is_sequence_header(&chunk) {
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

        self.process = Some(ffmpeg);

        // Wait for HLS playlist in background
        let app_clone = self.app.clone();
        tokio::spawn(async move {
            let playlist_path = config::hls_playlist_path(&app_clone);
            let mut attempts = 0;

            while !playlist_path.exists() && attempts < 50 {
                tokio::time::sleep(Duration::from_millis(500)).await;
                attempts += 1;
            }

            if playlist_path.exists() {
                println!("✅ Encoder started successfully");
                let _ = app_clone.emit(AppEvents::StreamPreviewActive.as_str(), ());
            } else {
                eprintln!("⚠️ Encoder failed to create HLS stream");
                let _ = app_clone.emit(AppEvents::StreamPreviewFailed.as_str(), ());
            }
        });

        let _ = self.app.emit(AppEvents::StreamActive.as_str(), ());
        println!("🎥 Encoder started");

        Ok(())
    }

    /// Write data to encoder (non-blocking via channel)
    pub async fn write_data(&self, data: Bytes) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(tx) = &self.write_tx {
            tx.send(data).await?;
        }
        Ok(())
    }

    /// Stop the encoder gracefully
    pub async fn stop(&mut self) {
        // Close write channel
        self.write_tx = None;

        if let Some(mut child) = self.process.take() {
            match child.wait().await {
                Ok(status) => {
                    println!("🛑 Encoder process exited with status: {}", status);
                }
                Err(e) => {
                    eprintln!("⚠️ Failed to wait on encoder process: {}", e);
                }
            }
        }

        let _ = self.app.emit(AppEvents::StreamPreviewEnded.as_str(), ());
    }

    /// Check if encoder is running
    pub fn is_running(&self) -> bool {
        self.process.is_some()
    }
}

// Helper to detect sequence headers
fn is_sequence_header(data: &[u8]) -> bool {
    if data.len() < 2 {
        return false;
    }
    // Check for AVC/AAC sequence header markers
    (data[0] == 0x17 && data[1] == 0x00) || // Video keyframe AVC sequence header
    (data[0] == 0xaf && data[1] == 0x00) // Audio AAC sequence header
}
