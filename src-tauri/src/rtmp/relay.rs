use super::utils::flv_header;

use crate::{
    config::{self, RelayHandle},
    db::{self},
    events::AppEvents,
};
use std::{
    process::Stdio,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};
use tauri::{AppHandle, Emitter, Manager};
use tokio::{
    io::AsyncWriteExt,
    process::Command,
    sync::{mpsc, Mutex},
};

pub async fn start_relay(app: &AppHandle, relay: &db::RelayTarget) {
    let state = app.state::<Arc<config::AppState>>();
    let mut relays = state.relays.lock().await;

    if relays.contains_key(&relay.id) {
        eprintln!("⚠️ Relay id:{} already exists", relay.id);
        return;
    }
    match spawn_relay(app, relay).await {
        Ok(handle) => {
            relays.insert(relay.id, handle);

            app.emit(AppEvents::RelayActive.as_str(), relay.id)
                .unwrap_or_else(|_| {
                    eprintln!("⚠️ Failed to emit active event for relay id:{}", relay.id);
                });
            println!("🟢 Started relay id:{}", relay.id);
        }
        Err(e) => eprintln!("❌ Failed to start relay id:{}: {}", relay.id, e),
    }
}

pub async fn stop_relay(app: &AppHandle, id: i64) {
    let state = app.state::<Arc<config::AppState>>();
    let mut relays = state.relays.lock().await;
    if let Some(handle) = relays.remove(&id) {
        handle.rx_task.abort();
        state.unregister_relay_channel(id).await;
        let _ = handle.process.lock().await.kill().await;
        app.emit(AppEvents::RelayEnded.as_str(), id)
            .unwrap_or_else(|_| {
                eprintln!("⚠️ Failed to emit end event for relay id:{}", id);
            });
        println!("🛑 Stopped relay id: {}", id);
    }
}

pub async fn start_relays(app: &AppHandle) {
    let pool = db::get_db_pool();
    let targets = db::get_active_relay_targets(pool).await.unwrap_or_default();
    print!("{:?}", targets);
    for relay in targets {
        start_relay(app, &relay).await;
    }
}

pub async fn stop_relays(app: &AppHandle) {
    let state = app.state::<Arc<config::AppState>>();
    let mut relays = state.relays.lock().await;
    for (_, handle) in relays.iter() {
        stop_relay(app, handle.id).await;
    }
    relays.clear();
}

async fn spawn_relay(
    app: &AppHandle,
    relay: &db::RelayTarget,
) -> Result<RelayHandle, Box<dyn std::error::Error>> {
    let id = relay.id;
    let log_dir = config::log_output_dir(app);
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
            &format!("{}/{}", relay.url, relay.stream_key),
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(log_file)
        .spawn()?;

    let mut stdin = child.stdin.take().unwrap();
    let shared_child = Arc::new(Mutex::new(child));
    let state = app.state::<Arc<config::AppState>>();
    let headers = state.encoder_sequence_headers.lock().await.clone();
    stdin.write_all(&flv_header()).await?;
    for tag in headers {
        stdin.write_all(&tag).await?;
    }

    let (tx, mut rx) = mpsc::channel(4096);
    // let tx_clone = tx.clone();
    state.register_relay_channel(id, tx).await;
    let task = tokio::spawn(async move {
        while let Some(data) = rx.recv().await {
            if let Err(e) = stdin.write_all(&data).await {
                eprintln!("⚠️ Relay write failed: {}", e);
                break;
            }
        }
    });
    // Task to monitor child process
    let id_clone = id;
    let child_monitor = shared_child.clone();
    let retrying = Arc::new(AtomicBool::new(false));
    // let relay_clone = relay.clone();
    let retrying_clone = retrying.clone();
    let app_clone = app.clone();
    tokio::spawn(async move {
        let mut child = child_monitor.lock().await;
        match child.wait().await {
            Ok(status) if status.success() => {
                println!("✅ Relay {} exited normally", id_clone);
                let _ = app_clone.emit(AppEvents::RelayEnded.as_str(), id_clone);
            }
            Ok(status) => {
                eprintln!("❌ Relay {} exited with code {:?}", id_clone, status.code());
                let _ = app_clone.emit(
                    AppEvents::RelayFailed.as_str(),
                    (id_clone, format!("Exited with code {:?}", status.code())),
                );
                if !retrying_clone.swap(true, Ordering::SeqCst) {
                    tokio::time::sleep(Duration::from_secs(3)).await;

                    // Retry spawn
                    // spawn_relay(&app_clone, &relay_clone).await;

                    println!("🔁 Relay {} restarted", id_clone);

                    retrying_clone.store(false, Ordering::SeqCst);
                }
            }
            Err(e) => {
                eprintln!("❌ Failed to wait on relay {}: {}", id_clone, e);
                let _ = app_clone.emit(
                    AppEvents::RelayFailed.as_str(),
                    (id_clone, format!("Wait error: {}", e)),
                );
            }
        }
    });

    Ok(config::RelayHandle {
        id,
        process: shared_child,
        rx_task: task,
        // tx,
    })
}
