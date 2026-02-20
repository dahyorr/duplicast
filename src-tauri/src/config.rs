use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use bytes::Bytes;
use get_if_addrs::get_if_addrs;
use rml_rtmp::sessions::StreamMetadata;
use serde::Serialize;
use sqlx::{prelude::FromRow, SqlitePool};
use tauri::{AppHandle, Manager};
use tokio::{
    net::TcpListener,
    process::{Child, ChildStdin},
    sync::{broadcast, Mutex},
};

use crate::{models::EncoderSettings, rtmp::relay::RelayHandle};

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct PortInfo {
    pub rtmp_port: u16,
    pub file_port: u16,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct StartUpData {
    pub ports: PortInfo,
    pub ips: Vec<String>,
}

pub struct AppState {
    pub rtmp_ready: Arc<AtomicBool>,
    pub file_ready: Arc<AtomicBool>,
    pub rtmp_active: AtomicBool,
    pub source_active: Arc<AtomicBool>,
    pub source_metadata: Mutex<Option<StreamMetadata>>,
    pub ports: Arc<Mutex<PortInfo>>,
    pub relays: Mutex<HashMap<i64, RelayHandle>>,
    pub encoder_process: Mutex<Option<Child>>,
    pub encoder_stdin: Mutex<Option<ChildStdin>>,
    pub encoder_sequence_headers: Mutex<Vec<Bytes>>,
    pub encoder_settings: Mutex<EncoderSettings>,
    pub encoder_tx: broadcast::Sender<Bytes>,
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
            source_metadata: Mutex::new(None),
            encoder_sequence_headers: Mutex::new(vec![]),
            encoder_settings: Mutex::new(EncoderSettings::new()),
            encoder_tx: broadcast::channel(16384).0,
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
    get_data_dir(app).join("hls_output")
}
pub fn log_output_dir(app: &AppHandle) -> PathBuf {
    get_data_dir(app).join("logs")
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

pub fn preflight_config(app: &AppHandle) {
    let hls_output_dir = hls_output_dir(app);
    let log_output_dir = log_output_dir(app);
    std::fs::create_dir_all(log_output_dir).unwrap_or_default();
    // delete hld_output_dir contents
    if hls_output_dir.exists() {
        clear_folder(&hls_output_dir).unwrap_or_default();
    } else {
        std::fs::create_dir_all(&hls_output_dir).unwrap_or_default();
    }
}

pub fn clear_folder(path: &Path) -> std::io::Result<()> {
    if path.is_dir() {
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let entry_path = entry.path();

            if entry_path.is_dir() {
                fs::remove_dir_all(&entry_path)?;
            } else {
                fs::remove_file(&entry_path)?;
            }
        }
    }
    Ok(())
}
