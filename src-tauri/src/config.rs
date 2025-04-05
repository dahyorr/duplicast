use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use get_if_addrs::get_if_addrs;
use rml_rtmp::sessions::StreamMetadata;
use serde::Serialize;
use sqlx::{prelude::FromRow, SqlitePool};
use tauri::{AppHandle, Manager};
use tokio::{
    net::TcpListener,
    process::{Child, ChildStdin},
    sync::{mpsc, Mutex},
    task::JoinHandle,
};

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct PortInfo {
    pub rtmp_port: u16,
    pub file_port: u16,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct StartUpData{
    pub ports: PortInfo,
    pub ips: Vec<String>,
}

#[derive(Debug)]
pub struct AppState {
    pub rtmp_ready: Arc<AtomicBool>,
    pub file_ready: Arc<AtomicBool>,
    pub rtmp_active: AtomicBool,
    pub source_active: Arc<AtomicBool>,
    pub source_metadata: Mutex<Option<StreamMetadata>>,
    pub ports: Arc<Mutex<PortInfo>>,
    pub relays: Mutex<HashMap<i64, RelayHandle>>,
    pub relay_channels: Mutex<HashMap<i64, mpsc::Sender<Arc<Vec<u8>>>>>,
    pub encoder_process: Mutex<Option<Child>>,
    pub encoder_stdin: Mutex<Option<ChildStdin>>,
    pub encoder_sequence_headers: Mutex<Vec<Vec<u8>>>,
    // pub metadata:
}

#[derive(Debug)]
pub struct RelayHandle {
    pub id: i64,
    pub process: Arc<Mutex<Child>>,
    pub rx_task: JoinHandle<()>,
    // pub tx: mpsc::Sender<Arc<Vec<u8>>>,
}

impl AppState {
    pub fn new(rtmp_port: u16, file_port: u16) -> Self {
        Self {
            rtmp_ready: Arc::new(AtomicBool::new(false)),
            source_active: Arc::new(AtomicBool::new(false)),
            rtmp_active: AtomicBool::new(false),
            file_ready: Arc::new(AtomicBool::new(false)),
            ports: Arc::new(Mutex::new(PortInfo {
                rtmp_port,
                file_port,
            })),
            relays: Mutex::new(HashMap::new()),
            encoder_process: Mutex::new(None),
            encoder_stdin: Mutex::new(None),
            relay_channels: Mutex::new(HashMap::new()),
            source_metadata: Mutex::new(None),
            encoder_sequence_headers: Mutex::new(vec![]),
        }
    }

    pub fn is_ready(&self) -> bool {
        self.rtmp_ready.load(Ordering::SeqCst) && self.file_ready.load(Ordering::SeqCst)
    }

    pub async fn register_relay_channel(&self, id: i64, tx: mpsc::Sender<Arc<Vec<u8>>>) {
        let mut relay_channels = self.relay_channels.lock().await;
        relay_channels.insert(id, tx);
    }

    pub async fn unregister_relay_channel(&self, id: i64) {
        let mut relay_channels = self.relay_channels.lock().await;
        relay_channels.remove(&id);
    }
}

async fn find_available_port(start_port: u16) -> Result<u16, Box<dyn std::error::Error>> {
    for port in start_port..=65535 {
        if TcpListener::bind(("127.0.0.1", port)).await.is_ok() {
            return Ok(port);
        }
    }
    panic!("⚠️ No available ports found");
}

pub async fn get_ip_addresses() -> Vec<String> {
    let mut ips = vec![];
    let max_ips = 3;
    // get max of 3 ipv4 addresses
    let interfaces = get_if_addrs().unwrap();
    for iface in interfaces {
        if iface.ip().is_ipv4() {
            ips.push(iface.ip().to_string());
            if ips.len() >= max_ips {
                break;
            }
        }
    }
    ips
}

pub async fn get_or_init_ports(pool: &SqlitePool) -> Result<PortInfo, Box<dyn std::error::Error>> {
    // Try reading existing config
    if let Some(config) =
        sqlx::query_as::<_, PortInfo>("SELECT rtmp_port, file_port FROM port_config LIMIT 1")
            .fetch_optional(pool)
            .await?
    {
        return Ok(config);
    }

    // Otherwise find available ports
    let rtmp_port = find_available_port(1580).await?;
    let file_port = find_available_port(8787).await?;

    sqlx::query("INSERT INTO port_config (rtmp_port, file_port) VALUES (?, ?)")
        .bind(rtmp_port)
        .bind(file_port)
        .execute(pool)
        .await?;

    Ok(PortInfo {
        rtmp_port,
        file_port,
    })
}

// store preview output path

pub fn get_data_dir(app: &AppHandle) -> PathBuf {
    let data_dir = app
        .path()
        .app_local_data_dir()
        .unwrap_or_else(|_| std::env::current_dir().unwrap());
    data_dir
}
pub fn hls_output_dir(app: &AppHandle) -> PathBuf {
    get_data_dir(app).join("./hls_output")
}
pub fn log_output_dir(app: &AppHandle) -> PathBuf {
    get_data_dir(app).join("./logs")
}
pub fn hls_playlist_path(app: &AppHandle) -> PathBuf {
    hls_output_dir(app).join("playlist.m3u8")
}

pub fn mask_key(key: &str) -> String {
    if key.len() <= 4 {
        "*".repeat(key.len())
    } else {
        let visible = &key[key.len() - 4..];
        format!("{}{}", "*".repeat(key.len() - 4), visible)
    }
}
