use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Stream {
    pub id: Uuid,
    pub stream_key: String,
    pub app_name: String,
    pub publisher_addr: String,
    pub started_at: SystemTime,
    pub bitrate: BitrateStats,
    pub status: StreamStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StreamStatus {
    Active,
    Inactive,
    Error(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BitrateStats {
    pub video_bitrate: u64,    // bits per second
    pub audio_bitrate: u64,    // bits per second
    pub total_bitrate: u64,    // bits per second
    pub total_bytes: u64,
    pub packets_received: u64,
    pub last_updated: Option<SystemTime>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Relay {
    pub id: Uuid,
    pub name: String,
    pub target_url: String,
    pub stream_id: Option<Uuid>,
    pub status: RelayStatus,
    pub created_at: SystemTime,
    pub started_at: Option<SystemTime>,
    pub stopped_at: Option<SystemTime>,
    pub bytes_sent: u64,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RelayStatus {
    Idle,
    Connecting,
    Active,
    Stopped,
    Error(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateRelayRequest {
    pub name: String,
    pub target_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartRelayRequest {
    pub stream_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamInfo {
    pub stream: Stream,
    pub relays: Vec<Relay>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebRTCOffer {
    pub sdp: String,
    #[serde(rename = "type")]
    pub type_: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebRTCAnswer {
    pub sdp: String,
    #[serde(rename = "type")]
    pub type_: String,
}

impl BitrateStats {
    pub fn update(&mut self, bytes: u64) {
        self.total_bytes += bytes;
        self.packets_received += 1;
        
        if let Some(last_updated) = self.last_updated {
            if let Ok(duration) = SystemTime::now().duration_since(last_updated) {
                if duration >= Duration::from_secs(1) {
                    // Calculate bitrate over the last second
                    let bytes_in_period = bytes;
                    self.total_bitrate = (bytes_in_period * 8) as u64; // Convert to bits
                    self.last_updated = Some(SystemTime::now());
                }
            }
        } else {
            self.last_updated = Some(SystemTime::now());
        }
    }
}
