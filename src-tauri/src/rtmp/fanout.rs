use crate::config::AppState;
use std::sync::Arc;
use tauri::{AppHandle, Manager};
use tokio::io::AsyncReadExt;

use super::utils::{is_audio_aac_sequence_header, is_video_keyframe_avc_sequence_header};

pub async fn start_fanout(
    app: AppHandle,
    mut stdout: tokio::process::ChildStdout,
) {
    let state = app.state::<Arc<AppState>>();
    let mut buf = [0u8; 4096];

    loop {
        match stdout.read(&mut buf).await {
            Ok(0) => {
                eprintln!("🔚 Encoder stdout closed");
                break;
            }
            Ok(n) => {
                let chunk = Arc::new(buf[..n].to_vec());
                let mut headers = state.encoder_sequence_headers.lock().await;
                if is_video_keyframe_avc_sequence_header(&chunk) {
                    println!("✅ Encoder video sequence header received");
                    headers.push(chunk.to_vec());
                }
                if is_audio_aac_sequence_header(&chunk) {
                    println!("✅ Encoder audio sequence header received");
                    headers.push(chunk.to_vec());
                }
                let relay_channels = state.relay_channels.lock().await;
                for (id, tx) in relay_channels.iter() {
                    if let Err(e) = tx.send(Arc::clone(&chunk)).await {
                        eprintln!("⚠️ Failed to send to relay {}: {}", id, e);
                    }
                }
            }
            Err(e) => {
                eprintln!("❌ Error reading encoder stdout: {}", e);
                break;
            }
        }
    }
}
