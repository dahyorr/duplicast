use crate::config::{self, RelayHandle};
use crate::db::{self};
use std::{process::Stdio, sync::Arc};
use tauri::{AppHandle, Manager};
use tokio::process::Command;

pub async fn start_relay(state: &Arc<config::AppState>, relay: &db::RelayTarget) {
    let mut relays = state.relays.lock().await;

    if relays.contains_key(&relay.id) {
        eprintln!("⚠️ Relay  id:{} already exists", relay.id);
        return;
    }
    match spawn_relay(relay.id, &relay.url, &relay.stream_key).await {
        Ok(handle) => {
            relays.insert(relay.id, handle);
            println!("🟢 Started relay id:{}", relay.id);
        }
        Err(e) => eprintln!("❌ Failed to start relay id:{}: {}", relay.id, e),
    }
}

pub async fn stop_relay(app: &AppHandle, id: i64) {
    let state = app.state::<Arc<config::AppState>>();
    let mut relays = state.relays.lock().await;
    if let Some(mut handle) = relays.remove(&id) {
        if let Err(e) = handle.process.kill().await {
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
    for (_, handle) in relays.iter_mut() {
        if let Err(e) = handle.process.kill().await {
            eprintln!("⚠️ Failed to kill relay process: {}", e);
        }
    }
    relays.clear();
}

async fn spawn_relay(
    id: i64,
    target_url: &str,
    stream_key: &str,
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

    let stdin = child.stdin.take().unwrap();

    Ok(config::RelayHandle {
        id,
        process: child,
        stdin,
    })
}
