use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use serde::Serialize;
use sqlx::{prelude::FromRow, SqlitePool};
use tokio::{
    net::TcpListener,
    process::Child,
    sync::{broadcast, Mutex},
};

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct PortInfo {
    pub rtmp_port: u16,
    pub file_port: u16,
}

#[derive(Debug)]
pub struct AppState {
    pub rtmp_ready: Arc<AtomicBool>,
    pub file_ready: Arc<AtomicBool>,
    pub rtmp_active: AtomicBool,
    pub source_active: Arc<AtomicBool>,
    pub ports: Arc<Mutex<PortInfo>>,
    pub relays: Mutex<HashMap<i64, Child>>,
    pub relay_tx: broadcast::Sender<Vec<u8>>,
    pub preview_task: Mutex<Option<tokio::task::JoinHandle<()>>>,
}

impl AppState {
    pub fn new(rtmp_port: u16, file_port: u16) -> Self {
        let (relay_tx, _) = broadcast::channel::<Vec<u8>>(128); // capacity can be tuned
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
            relay_tx,
            preview_task: Mutex::new(None),
        }
    }

    pub fn is_ready(&self) -> bool {
        self.rtmp_ready.load(Ordering::SeqCst) && self.file_ready.load(Ordering::SeqCst)
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
pub fn hls_output_dir() -> PathBuf {
    PathBuf::from("./hls_output")
}
pub fn hls_playlist_path() -> PathBuf {
    hls_output_dir().join("playlist.m3u8")
}

pub fn mask_key(key: &str) -> String {
    if key.len() <= 4 {
        "*".repeat(key.len())
    } else {
        let visible = &key[key.len() - 4..];
        format!("{}{}", "*".repeat(key.len() - 4), visible)
    }
}
