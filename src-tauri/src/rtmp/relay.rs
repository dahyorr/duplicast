use super::utils::flv_header;

use crate::config::{self, RelayHandle};
use crate::db::{self};
use std::{process::Stdio, sync::Arc};
use tokio::{io::AsyncWriteExt, process::Command};

pub async fn start_relay(state: &Arc<config::AppState>, relay: &db::RelayTarget) {
    let mut relays = state.relays.lock().await;

    if relays.contains_key(&relay.id) {
        eprintln!("⚠️ Relay id:{} already exists", relay.id);
        return;
    }
    match spawn_relay(relay.id, &relay.url, &relay.stream_key, state).await {
        Ok(handle) => {
            relays.insert(relay.id, handle);
            println!("🟢 Started relay id:{}", relay.id);
        }
        Err(e) => eprintln!("❌ Failed to start relay id:{}: {}", relay.id, e),
    }
}

pub async fn stop_relay(state: &Arc<config::AppState>, id: i64) {
    let mut relays = state.relays.lock().await;
    if let Some(mut handle) = relays.remove(&id) {
        handle.rx_task.abort();
        let _ = handle.process.kill().await;
        println!("🛑 Stopped relay id: {}", id);
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

pub async fn stop_relays(state: &Arc<config::AppState>) {
    let mut relays = state.relays.lock().await;
    for (_, handle) in relays.iter() {
        stop_relay(state, handle.id).await;
    }
    relays.clear();
}

async fn spawn_relay(
    id: i64,
    target_url: &str,
    stream_key: &str,
    state: &Arc<config::AppState>,
) -> Result<RelayHandle, Box<dyn std::error::Error>> {
    let log_dir = config::log_output_dir();
    let log_file = std::fs::File::create(log_dir.join(format!("relay_{id}.log")))?;
    let log_file = Stdio::from(log_file);

    let mut child = Command::new("ffmpeg")
        .args([
            "-f",
            "flv",
            "-i",
            "pipe:0",
            "-c:v",
            "copy",
            "-c:a",
            "copy",
            "-f",
            "flv",
            &format!("{}/{}", target_url, stream_key),
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(log_file)
        .spawn()?;

    let mut stdin = child.stdin.take().unwrap();
    let encoder_tx = state.encoder_tx.clone();
    let mut rx = encoder_tx.subscribe();

    let headers = state.encoder_sequence_headers.lock().await.clone();
    stdin.write_all(&flv_header()).await?;
    for tag in headers {
        println!("{:?}", tag);
        stdin.write_all(&tag).await?;
    }
    let task = tokio::spawn(async move {
        while let Ok(chunk) = rx.recv().await {
            if let Err(e) = stdin.write_all(&chunk).await {
                eprintln!("⚠️ Relay write failed: {}", e);
                break;
            }
        }
    });

    Ok(config::RelayHandle {
        id,
        process: child,
        rx_task: task,
    })
}
