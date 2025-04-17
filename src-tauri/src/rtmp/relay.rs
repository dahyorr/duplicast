use super::utils::flv_header;

use crate::{config, db, events::AppEvents, models};
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
    process::{Child, Command},
    sync::{broadcast, Mutex},
    task::JoinHandle,
};

#[derive(Debug)]
struct RelayCredentials {
    url: String,
    stream_key: String,
}

#[derive(Debug)]
pub struct RelayHandle {
    pub id: i64,
    pub process: Option<Arc<Mutex<Child>>>,
    pub active: Arc<AtomicBool>,
    pub retrying: Arc<AtomicBool>,
    credentials: RelayCredentials,
    pub rx: broadcast::Receiver<Vec<u8>>,
    pub rx_task: Option<JoinHandle<()>>,
    // pub tx: mpsc::Sender<Arc<Vec<u8>>>,
}

impl RelayHandle {
    pub fn from_relay_target(
        relay: &models::RelayTarget,
        encoder_rx: broadcast::Receiver<Vec<u8>>,
    ) -> Self {
        Self {
            id: relay.id,
            credentials: RelayCredentials {
                url: relay.url.clone(),
                stream_key: relay.stream_key.clone(),
            },
            active: Arc::new(AtomicBool::new(false)),
            process: None,
            rx: encoder_rx,
            rx_task: None,
            retrying: Arc::new(AtomicBool::new(false)),
        }
    }

    pub async fn start(&mut self, app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
        let state = app.state::<Arc<config::AppState>>();
        let log_dir = config::log_output_dir(app);
        let log_file = std::fs::File::create(log_dir.join(format!("relay_{}.log", self.id)))?;
        let log_file = Stdio::from(log_file);
        println!("🔄 Starting relay {}", self.id);

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
                &format!("{}/{}", self.credentials.url, self.credentials.stream_key),
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(log_file)
            .spawn()?;

        let mut stdin = child.stdin.take().unwrap();
        let shared_child = Arc::new(Mutex::new(child));
        let headers = state.encoder_sequence_headers.lock().await.clone();
        stdin.write_all(&flv_header()).await?;
        for tag in headers {
            stdin.write_all(&tag).await?;
        }

        let mut rx = self.rx.resubscribe();
        let id_write_task = self.id.clone();
        let write_task = tokio::spawn(async move {
            while let Ok(data) = rx.recv().await {
                if stdin.write_all(&data).await.is_err() {
                    eprintln!("⚠️ Relay {} write failed", id_write_task);
                    break;
                }
            }
        });

        let id_clone = self.id;
        let retrying_clone = self.retrying.clone();
        let app_clone = app.clone();
        let child_monitor = shared_child.clone();

        tokio::spawn(async move {
            let mut child = child_monitor.lock().await;
            match child.wait().await {
                Ok(status) if status.success() => {
                    let _ = app_clone.emit(AppEvents::RelayEnded.as_str(), id_clone);
                }
                Ok(status) => {
                    let _ = app_clone.emit(
                        AppEvents::RelayFailed.as_str(),
                        (id_clone, format!("Exited with code {:?}", status.code())),
                    );
                    if !retrying_clone.swap(true, Ordering::SeqCst) {
                        tokio::time::sleep(Duration::from_secs(3)).await;
                        // Optionally trigger restart logic here
                        retrying_clone.store(false, Ordering::SeqCst);
                    }
                }
                Err(e) => {
                    let _ = app_clone.emit(
                        AppEvents::RelayFailed.as_str(),
                        (id_clone, format!("Wait error: {}", e)),
                    );
                }
            }
        });

        self.process = Some(shared_child);
        self.rx_task = Some(write_task);
        self.active.store(true, Ordering::SeqCst);

        app.emit(AppEvents::RelayActive.as_str(), self.id)
            .unwrap_or_else(|_| {
                eprintln!("⚠️ Failed to emit active event for relay id:{}", self.id);
            });

        println!("🟢 Relay {} started", self.id);
        Ok(())
    }

    pub async fn stop(&mut self, app: &AppHandle) {
        if self.active.load(Ordering::SeqCst) {
            if let Some(task) = self.rx_task.take() {
                task.abort();
            }
            if let Some(handle) = self.process.take() {
                handle.lock().await.kill().await.ok();
            }
            let _ = app.emit(AppEvents::RelayEnded.as_str(), self.id);
            self.active.store(false, Ordering::SeqCst);
        }
    }
}

pub async fn start_relays(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let pool = db::get_db_pool();
    let relays = db::get_active_relay_targets(&pool).await?;
    let state = app.state::<Arc<config::AppState>>();
    let tx = state.encoder_tx.clone();

    for relay in relays {
        let mut relay_handle = RelayHandle::from_relay_target(&relay, tx.subscribe());
        relay_handle.start(app).await?;
        state.relays.lock().await.insert(relay.id, relay_handle);
    }
    Ok(())
}

pub async fn stop_relays(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let state = app.state::<Arc<config::AppState>>();
    let mut relays = state.relays.lock().await;

    for relay in relays.values_mut() {
        relay.stop(app).await;
    }
    Ok(())
}
