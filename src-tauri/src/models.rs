use serde::{Deserialize, Serialize};
use sqlx::FromRow;

use crate::config;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct EncoderSettings {
    pub video_bitrate: u32,
    pub audio_bitrate: u32,
    pub video_codec: String,
    pub audio_codec: String,
    pub preset: String,
    pub tune: Option<String>,
    pub bufsize: Option<u32>,
    pub framerate: Option<u32>,
    pub resolution: Option<String>,
    pub use_passthrough: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct RelayTarget {
    pub id: i64,
    pub tag: String,
    pub stream_key: String,
    pub url: String,
    pub enabled: bool,
    pub created_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct RelayTargetPublic {
    pub id: i64,
    pub tag: String,
    pub stream_key: String,
    pub url: String,
    pub enabled: bool,
    pub created_at: Option<String>,
}

impl RelayTargetPublic {
    pub fn from_relay_target(relay_target: &RelayTarget) -> Self {
        Self {
            id: relay_target.id,
            tag: relay_target.tag.clone(),
            stream_key: config::mask_key(&relay_target.stream_key),
            url: relay_target.url.clone(),
            enabled: relay_target.enabled,
            created_at: relay_target.created_at.clone(),
        }
    }
}

impl EncoderSettings {
    pub fn new() -> Self {
        Self {
            video_bitrate: 3000,
            audio_bitrate: 160,
            video_codec: "libx264".into(),
            audio_codec: "aac".into(),
            preset: "veryfast".into(),
            tune: Some("zerolatency".into()),
            bufsize: Some(8000),
            framerate: None,
            resolution: None,
            use_passthrough: false,
        }
    }
}
