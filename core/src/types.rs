use serde::{Deserialize, Serialize, Serializer};
use std::time::{Duration, SystemTime};
use uuid::Uuid;

fn mask_key<S: Serializer>(key: &str, ser: S) -> Result<S::Ok, S::Error> {
    ser.serialize_str(if key.is_empty() { "" } else { "****" })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub listen_addr: String,
    pub rtmp_port: u16,
    pub max_bitrate_kbps: u64,
    pub relay_auto_reconnect: bool,
    pub relay_reconnect_delay_secs: u64,
    pub relay_reconnect_attempts: u32,
    pub log_level: String,
    pub health_check_interval_secs: u64,
    pub api_enabled: bool,
    pub api_port: u16,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            listen_addr: "0.0.0.0".to_string(),
            rtmp_port: 1935,
            max_bitrate_kbps: 8000,
            relay_auto_reconnect: true,
            relay_reconnect_delay_secs: 5,
            relay_reconnect_attempts: 10,
            log_level: "info".to_string(),
            health_check_interval_secs: 300,
            api_enabled: true,
            api_port: 8080,
        }
    }
}

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
    bytes_in_current_period: u64, // Track bytes in current measurement period
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Relay {
    pub id: Uuid,
    pub name: String,
    pub rtmp_url: String,
    #[serde(serialize_with = "mask_key")]
    pub stream_key: String,
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
    pub rtmp_url: String,
    #[serde(default)]
    pub stream_key: String,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IceCandidate {
    pub candidate: String,
    #[serde(rename = "sdpMLineIndex")]
    pub sdp_m_line_index: Option<u16>,
    #[serde(rename = "sdpMid")]
    pub sdp_mid: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LogLevel {
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub id: String,
    pub timestamp: String,
    pub level: LogLevel,
    pub message: String,
    pub source: String,
}

impl BitrateStats {
    pub fn update(&mut self, bytes: u64) {
        self.total_bytes += bytes;
        self.packets_received += 1;
        
        if let Some(last_updated) = self.last_updated {
            self.bytes_in_current_period += bytes;
            
            if let Ok(duration) = SystemTime::now().duration_since(last_updated) {
                if duration >= Duration::from_secs(1) {
                    // Calculate bitrate based on all bytes received in this period
                    self.total_bitrate = (self.bytes_in_current_period * 8) as u64; // Convert to bits
                    self.bytes_in_current_period = 0; // Reset for next period
                    self.last_updated = Some(SystemTime::now());
                }
            }
        } else {
            self.last_updated = Some(SystemTime::now());
            self.bytes_in_current_period = bytes;
        }
    }
}
