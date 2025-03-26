use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};

use serde::Serialize;
use sqlx::{prelude::FromRow, SqlitePool};
use tokio::net::TcpListener;

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct PortInfo {
    pub rtmp_port: u16,
    pub file_port: u16,
}

pub struct AppState {
    pub rtmp_ready: Arc<AtomicBool>,
    pub file_ready: Arc<AtomicBool>,
    pub ports: Arc<Mutex<PortInfo>>,
}

impl AppState {
    pub fn new(rtmp_port: u16, file_port: u16) -> Self {
        Self {
            rtmp_ready: Arc::new(AtomicBool::new(false)),
            file_ready: Arc::new(AtomicBool::new(false)),
            ports: Arc::new(Mutex::new(PortInfo {
                rtmp_port,
                file_port,
            })),
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
