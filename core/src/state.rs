use crate::types::{Relay, RelayStatus, Stream, StreamStatus};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::SystemTime;
use tokio::sync::RwLock;
use uuid::Uuid;

#[derive(Clone)]
pub struct AppState {
    pub streams: Arc<RwLock<HashMap<Uuid, Stream>>>,
    pub relays: Arc<RwLock<HashMap<Uuid, Relay>>>,
    pub stream_by_key: Arc<RwLock<HashMap<String, Uuid>>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            streams: Arc::new(RwLock::new(HashMap::new())),
            relays: Arc::new(RwLock::new(HashMap::new())),
            stream_by_key: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn register_stream(
        &self,
        stream_key: String,
        app_name: String,
        publisher_addr: String,
    ) -> Uuid {
        let stream_id = Uuid::new_v4();
        let stream = Stream {
            id: stream_id,
            stream_key: stream_key.clone(),
            app_name,
            publisher_addr,
            started_at: SystemTime::now(),
            bitrate: Default::default(),
            status: StreamStatus::Active,
        };

        let mut streams = self.streams.write().await;
        let mut stream_by_key = self.stream_by_key.write().await;

        streams.insert(stream_id, stream);
        stream_by_key.insert(stream_key, stream_id);

        stream_id
    }

    pub async fn unregister_stream(&self, stream_id: Uuid) {
        let mut streams = self.streams.write().await;
        let mut stream_by_key = self.stream_by_key.write().await;

        if let Some(stream) = streams.remove(&stream_id) {
            stream_by_key.remove(&stream.stream_key);
        }

        // Stop all relays for this stream
        let mut relays = self.relays.write().await;
        for relay in relays.values_mut() {
            if relay.stream_id == Some(stream_id) {
                relay.status = RelayStatus::Stopped;
                relay.stopped_at = Some(SystemTime::now());
                relay.stream_id = None;
            }
        }
    }

    pub async fn update_stream_bitrate(&self, stream_id: Uuid, bytes: u64) {
        let mut streams = self.streams.write().await;
        if let Some(stream) = streams.get_mut(&stream_id) {
            stream.bitrate.update(bytes);
        }
    }

    pub async fn create_relay(&self, name: String, target_url: String) -> Uuid {
        let relay_id = Uuid::new_v4();
        let relay = Relay {
            id: relay_id,
            name,
            target_url,
            stream_id: None,
            status: RelayStatus::Idle,
            created_at: SystemTime::now(),
            started_at: None,
            stopped_at: None,
            bytes_sent: 0,
            enabled: true,
        };

        let mut relays = self.relays.write().await;
        relays.insert(relay_id, relay);

        relay_id
    }

    pub async fn start_relay(&self, relay_id: Uuid, stream_id: Uuid) -> Result<(), String> {
        let mut relays = self.relays.write().await;
        let relay = relays
            .get_mut(&relay_id)
            .ok_or("Relay not found")?;

        if !relay.enabled {
            return Err("Relay is disabled".to_string());
        }

        relay.stream_id = Some(stream_id);
        relay.status = RelayStatus::Connecting;
        relay.started_at = Some(SystemTime::now());

        Ok(())
    }

    pub async fn stop_relay(&self, relay_id: Uuid) -> Result<(), String> {
        let mut relays = self.relays.write().await;
        let relay = relays
            .get_mut(&relay_id)
            .ok_or("Relay not found")?;

        relay.status = RelayStatus::Stopped;
        relay.stopped_at = Some(SystemTime::now());
        relay.stream_id = None;

        Ok(())
    }

    pub async fn delete_relay(&self, relay_id: Uuid) -> Result<(), String> {
        let mut relays = self.relays.write().await;
        relays.remove(&relay_id).ok_or("Relay not found")?;
        Ok(())
    }

    pub async fn get_all_streams(&self) -> Vec<Stream> {
        let streams = self.streams.read().await;
        streams.values().cloned().collect()
    }

    pub async fn get_stream(&self, stream_id: Uuid) -> Option<Stream> {
        let streams = self.streams.read().await;
        streams.get(&stream_id).cloned()
    }

    pub async fn get_all_relays(&self) -> Vec<Relay> {
        let relays = self.relays.read().await;
        relays.values().cloned().collect()
    }

    pub async fn get_relay(&self, relay_id: Uuid) -> Option<Relay> {
        let relays = self.relays.read().await;
        relays.get(&relay_id).cloned()
    }

    pub async fn get_relays_for_stream(&self, stream_id: Uuid) -> Vec<Relay> {
        let relays = self.relays.read().await;
        relays
            .values()
            .filter(|r| r.stream_id == Some(stream_id))
            .cloned()
            .collect()
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
