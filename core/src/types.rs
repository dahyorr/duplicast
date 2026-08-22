use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize, Serializer};
use uuid::Uuid;

fn mask_key<S: Serializer>(key: &str, ser: S) -> Result<S::Ok, S::Error> {
    ser.serialize_str(if key.is_empty() { "" } else { "****" })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub rtmp_port: u16,
    pub relay_auto_reconnect: bool,
    pub relay_reconnect_delay_secs: u64,
    pub relay_reconnect_attempts: u32,
    pub api_port: u16,
    /// host:port of the STUN server used for WebRTC preview NAT traversal.
    pub stun_server: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            rtmp_port: 1935,
            relay_auto_reconnect: true,
            relay_reconnect_delay_secs: 5,
            relay_reconnect_attempts: 10,
            api_port: 8080,
            stun_server: "stun.l.google.com:19302".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Stream {
    pub id: Uuid,
    pub stream_key: String,
    pub app_name: String,
    pub publisher_addr: String,
    pub started_at: DateTime<Utc>,
    pub bitrate: BitrateStats,
    pub status: StreamStatus,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", content = "message", rename_all = "lowercase")]
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
    pub last_updated: Option<DateTime<Utc>>,
    #[serde(skip)]
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
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub stopped_at: Option<DateTime<Utc>>,
    pub bytes_sent: u64,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", content = "message", rename_all = "lowercase")]
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
    pub session_id: String,
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

            let elapsed = Utc::now().signed_duration_since(last_updated);
            if elapsed >= chrono::Duration::seconds(1) {
                // Calculate bitrate based on all bytes received in this period
                self.total_bitrate = self.bytes_in_current_period * 8; // Convert to bits
                self.bytes_in_current_period = 0; // Reset for next period
                self.last_updated = Some(Utc::now());
            }
        } else {
            self.last_updated = Some(Utc::now());
            self.bytes_in_current_period = bytes;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_unit_variant_serializes_as_bare_status_string() {
        let json = serde_json::to_value(StreamStatus::Active).unwrap();
        assert_eq!(json, serde_json::json!({"status": "active"}));

        let json = serde_json::to_value(RelayStatus::Idle).unwrap();
        assert_eq!(json, serde_json::json!({"status": "idle"}));
    }

    #[test]
    fn status_error_variant_serializes_with_message() {
        let json = serde_json::to_value(RelayStatus::Error("connection lost".to_string())).unwrap();
        assert_eq!(
            json,
            serde_json::json!({"status": "error", "message": "connection lost"})
        );
    }

    #[test]
    fn status_round_trips_through_json() {
        for status in [
            RelayStatus::Idle,
            RelayStatus::Connecting,
            RelayStatus::Active,
            RelayStatus::Stopped,
            RelayStatus::Error("boom".to_string()),
        ] {
            let json = serde_json::to_string(&status).unwrap();
            let back: RelayStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(status, back);
        }
    }

    #[test]
    fn timestamp_fields_serialize_as_rfc3339_strings() {
        let stream = Stream {
            id: Uuid::new_v4(),
            stream_key: "key".to_string(),
            app_name: "app".to_string(),
            publisher_addr: "127.0.0.1:1935".to_string(),
            started_at: Utc::now(),
            bitrate: BitrateStats::default(),
            status: StreamStatus::Active,
        };

        let json = serde_json::to_value(&stream).unwrap();
        let started_at = json.get("started_at").unwrap();
        assert!(started_at.is_string());
        // Must parse back as a valid RFC3339 timestamp, not a {secs,nanos} object.
        DateTime::parse_from_rfc3339(started_at.as_str().unwrap()).unwrap();
    }
}
