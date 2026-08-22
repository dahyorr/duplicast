use crate::pipeline::PipelineSetup;
use crate::types::{Config, LogEntry, LogLevel, Relay, RelayStatus, Stream, StreamStatus};
use bytes::Bytes;
use gstreamer::prelude::{ElementExt, GstBinExt, PadExt, PadExtManual, *};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use chrono::Utc;
use std::sync::{Arc, Mutex};
use tokio::sync::RwLock;
use uuid::Uuid;

fn config_file_path() -> std::path::PathBuf {
    if let Ok(path) = std::env::var("DUPLICAST_CONFIG_PATH") {
        return std::path::PathBuf::from(path);
    }
    // Fall back to a path next to the executable so it doesn't depend on CWD
    // when launched from e.g. systemd, while still keeping local `cargo run`
    // convenient (target/debug/duplicast_config.json).
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("duplicast_config.json")))
        .unwrap_or_else(|| std::path::PathBuf::from("duplicast_config.json"))
}

fn load_config_sync() -> Config {
    std::fs::read_to_string(config_file_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

// Per-stream GStreamer pipeline handles.
pub struct StreamPipeline {
    pub pipeline: gstreamer::Pipeline,
    pub videotee: gstreamer::Element,
    pub audiotee: gstreamer::Element,
    pub video_bytes: Arc<AtomicU64>,
    pub audio_bytes: Arc<AtomicU64>,
}

// GStreamer elements owned by one WebRTC viewer session.
pub struct WebRTCSession {
    pub stream_id: Uuid,
    pub pipeline: gstreamer::Pipeline,
    pub videotee: gstreamer::Element,
    pub audiotee: gstreamer::Element,
    pub vtee_src_pad: gstreamer::Pad,
    pub atee_src_pad: gstreamer::Pad,
    // Every element added to the pipeline for this session (for bulk removal).
    pub elements: Vec<gstreamer::Element>,
}

// GStreamer elements owned by one active relay. Stored so we can detach cleanly.
pub struct RelayElements {
    pub videotee: gstreamer::Element,
    pub audiotee: gstreamer::Element,
    pub videotee_src_pad: gstreamer::Pad,
    pub audiotee_src_pad: gstreamer::Pad,
    pub queue_video: gstreamer::Element,
    pub queue_audio: gstreamer::Element,
    pub h264parse_relay: gstreamer::Element,
    pub aacparse_relay: gstreamer::Element,
    pub flvmux: gstreamer::Element,
    pub rtmpsink: gstreamer::Element,
    pub bytes_sent: Arc<AtomicU64>,
}

// Fans out the FLV-tagged bytes for one stream (the same bytes already pushed
// into the ingest appsrc) to any number of HTTP-FLV preview viewers. Sequence
// header tags (AVCDecoderConfigurationRecord / AudioSpecificConfig) are cached
// so a viewer that joins mid-stream still gets a decodable init segment.
pub struct FlvBroadcaster {
    pub tx: tokio::sync::broadcast::Sender<Bytes>,
    pub video_config: Mutex<Option<Bytes>>,
    pub audio_config: Mutex<Option<Bytes>>,
}

#[derive(Clone)]
pub struct AppState {
    pub streams: Arc<RwLock<HashMap<Uuid, Stream>>>,
    pub relays: Arc<RwLock<HashMap<Uuid, Relay>>>,
    pub stream_by_key: Arc<RwLock<HashMap<String, Uuid>>>,
    pub logs: Arc<RwLock<Vec<LogEntry>>>,
    pub pipelines: Arc<RwLock<HashMap<Uuid, StreamPipeline>>>,
    pub relay_elements: Arc<RwLock<HashMap<Uuid, RelayElements>>>,
    pub webrtc_sessions: Arc<RwLock<HashMap<String, WebRTCSession>>>,
    pub flv_broadcasters: Arc<RwLock<HashMap<Uuid, Arc<FlvBroadcaster>>>>,
    pub config: Arc<RwLock<Config>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            streams: Arc::new(RwLock::new(HashMap::new())),
            relays: Arc::new(RwLock::new(HashMap::new())),
            stream_by_key: Arc::new(RwLock::new(HashMap::new())),
            logs: Arc::new(RwLock::new(Vec::new())),
            pipelines: Arc::new(RwLock::new(HashMap::new())),
            relay_elements: Arc::new(RwLock::new(HashMap::new())),
            webrtc_sessions: Arc::new(RwLock::new(HashMap::new())),
            flv_broadcasters: Arc::new(RwLock::new(HashMap::new())),
            config: Arc::new(RwLock::new(load_config_sync())),
        }
    }

    pub async fn add_webrtc_session(&self, session_id: String, session: WebRTCSession) {
        let mut sessions = self.webrtc_sessions.write().await;
        sessions.insert(session_id, session);
    }

    pub async fn remove_webrtc_session(&self, session_id: &str) {
        let session = {
            let mut sessions = self.webrtc_sessions.write().await;
            sessions.remove(session_id)
        };
        if let Some(s) = session {
            tokio::task::spawn_blocking(move || detach_webrtc_from_pipeline(s))
                .await
                .ok();
        }
    }

    /// Tears down every active GStreamer pipeline (stream ingest + attached relay/WebRTC
    /// elements). Called on process shutdown so a restart doesn't leave dangling
    /// GStreamer subprocesses or half-written relay connections.
    pub async fn shutdown(&self) {
        let pipelines = self.pipelines.read().await;
        for pipeline in pipelines.values() {
            let _ = pipeline.pipeline.set_state(gstreamer::State::Null);
        }
    }

    pub async fn get_config(&self) -> Config {
        self.config.read().await.clone()
    }

    pub async fn update_config(&self, new_config: Config) -> anyhow::Result<()> {
        let json = serde_json::to_string_pretty(&new_config)?;
        tokio::fs::write(config_file_path(), json).await?;
        *self.config.write().await = new_config;
        Ok(())
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
            started_at: Utc::now(),
            bitrate: Default::default(),
            status: StreamStatus::Active,
        };

        let mut streams = self.streams.write().await;
        let mut stream_by_key = self.stream_by_key.write().await;

        streams.insert(stream_id, stream);
        stream_by_key.insert(stream_key, stream_id);

        let (tx, _rx) = tokio::sync::broadcast::channel(256);
        self.flv_broadcasters.write().await.insert(
            stream_id,
            Arc::new(FlvBroadcaster {
                tx,
                video_config: Mutex::new(None),
                audio_config: Mutex::new(None),
            }),
        );

        stream_id
    }

    /// Returns (flv_header, cached_video_config, cached_audio_config, live_receiver)
    /// for a new HTTP-FLV viewer, or None if the stream doesn't exist.
    pub async fn flv_subscribe(
        &self,
        stream_id: Uuid,
    ) -> Option<(Bytes, Option<Bytes>, Option<Bytes>, tokio::sync::broadcast::Receiver<Bytes>)> {
        let broadcasters = self.flv_broadcasters.read().await;
        let b = broadcasters.get(&stream_id)?;
        let video_config = b.video_config.lock().unwrap().clone();
        let audio_config = b.audio_config.lock().unwrap().clone();
        let rx = b.tx.subscribe();
        Some((Bytes::from(crate::flv::flv_header()), video_config, audio_config, rx))
    }

    /// Broadcasts one FLV-wrapped video tag to any subscribed HTTP-FLV viewers.
    /// Caches it if it's an AVC sequence header (SPS/PPS), so viewers who join
    /// mid-stream still get a decodable init segment.
    pub async fn flv_publish_video(&self, stream_id: Uuid, tag: Bytes, is_sequence_header: bool) {
        let broadcasters = self.flv_broadcasters.read().await;
        if let Some(b) = broadcasters.get(&stream_id) {
            if is_sequence_header {
                *b.video_config.lock().unwrap() = Some(tag.clone());
            }
            let _ = b.tx.send(tag);
        }
    }

    /// Same as `flv_publish_video` but for audio (AudioSpecificConfig sequence header).
    pub async fn flv_publish_audio(&self, stream_id: Uuid, tag: Bytes, is_sequence_header: bool) {
        let broadcasters = self.flv_broadcasters.read().await;
        if let Some(b) = broadcasters.get(&stream_id) {
            if is_sequence_header {
                *b.audio_config.lock().unwrap() = Some(tag.clone());
            }
            let _ = b.tx.send(tag);
        }
    }

    pub async fn register_pipeline(&self, stream_id: Uuid, setup: PipelineSetup) {
        let video_bytes = setup.video_bytes.clone();
        let audio_bytes = setup.audio_bytes.clone();

        {
            let mut pipelines = self.pipelines.write().await;
            pipelines.insert(
                stream_id,
                StreamPipeline {
                    pipeline: setup.pipeline,
                    videotee: setup.videotee,
                    audiotee: setup.audiotee,
                    video_bytes: setup.video_bytes,
                    audio_bytes: setup.audio_bytes,
                },
            );
        }

        // Background task: compute per-codec bitrates from pad probe byte counters.
        let state_clone = self.clone();
        tokio::spawn(async move {
            let mut prev_video = 0u64;
            let mut prev_audio = 0u64;
            let mut last = std::time::Instant::now();

            loop {
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;

                if state_clone.get_stream(stream_id).await.is_none() {
                    break;
                }

                let now = std::time::Instant::now();
                let elapsed = now.duration_since(last).as_secs_f64();
                last = now;

                let curr_video = video_bytes.load(Ordering::Relaxed);
                let curr_audio = audio_bytes.load(Ordering::Relaxed);

                let delta_video = curr_video.saturating_sub(prev_video);
                let delta_audio = curr_audio.saturating_sub(prev_audio);
                prev_video = curr_video;
                prev_audio = curr_audio;

                let video_bps = if elapsed > 0.0 {
                    ((delta_video as f64 / elapsed) * 8.0) as u64
                } else {
                    0
                };
                let audio_bps = if elapsed > 0.0 {
                    ((delta_audio as f64 / elapsed) * 8.0) as u64
                } else {
                    0
                };

                let mut streams = state_clone.streams.write().await;
                if let Some(stream) = streams.get_mut(&stream_id) {
                    stream.bitrate.video_bitrate = video_bps;
                    stream.bitrate.audio_bitrate = audio_bps;
                }
            }
        });
    }

    pub async fn unregister_stream(&self, stream_id: Uuid) {
        // Collect relay IDs for this stream before clearing their stream_id field.
        let relay_ids: Vec<Uuid> = {
            let relays = self.relays.read().await;
            relays
                .values()
                .filter(|r| r.stream_id == Some(stream_id))
                .map(|r| r.id)
                .collect()
        };

        // Remove stream records.
        {
            let mut streams = self.streams.write().await;
            let mut stream_by_key = self.stream_by_key.write().await;
            if let Some(stream) = streams.remove(&stream_id) {
                stream_by_key.remove(&stream.stream_key);
            }
        }

        // Mark all relays for this stream as stopped.
        {
            let mut relays = self.relays.write().await;
            for relay in relays.values_mut() {
                if relay.stream_id == Some(stream_id) {
                    relay.status = RelayStatus::Stopped;
                    relay.stopped_at = Some(Utc::now());
                    relay.stream_id = None;
                }
            }
        }

        // Drop relay element handles; the pipeline teardown below will clean up the GStreamer graph.
        {
            let mut relay_elements = self.relay_elements.write().await;
            for relay_id in &relay_ids {
                relay_elements.remove(relay_id);
            }
        }

        // Drop WebRTC session handles for this stream (pipeline teardown cleans up elements).
        {
            let mut sessions = self.webrtc_sessions.write().await;
            sessions.retain(|_, s| s.stream_id != stream_id);
        }

        // Destroy the stream's GStreamer pipeline.
        {
            let mut pipelines = self.pipelines.write().await;
            if let Some(sp) = pipelines.remove(&stream_id) {
                let _ = sp.pipeline.set_state(gstreamer::State::Null);
            }
        }

        // Drop the FLV broadcaster - closes the channel, which ends every
        // subscribed HTTP-FLV viewer's response stream.
        self.flv_broadcasters.write().await.remove(&stream_id);
    }

    pub async fn update_stream_bitrate(&self, stream_id: Uuid, bytes: u64) {
        let mut streams = self.streams.write().await;
        if let Some(stream) = streams.get_mut(&stream_id) {
            stream.bitrate.update(bytes);
        }
    }

    pub async fn create_relay(&self, name: String, rtmp_url: String, stream_key: String) -> Uuid {
        let relay_id = Uuid::new_v4();
        let relay = Relay {
            id: relay_id,
            name,
            rtmp_url,
            stream_key,
            stream_id: None,
            status: RelayStatus::Idle,
            created_at: Utc::now(),
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
        // Validate relay and build the full RTMP destination URL.
        let target_url = {
            let relays = self.relays.read().await;
            let relay = relays.get(&relay_id).ok_or("Relay not found")?;
            if !relay.enabled {
                return Err("Relay is disabled".to_string());
            }
            if matches!(relay.status, RelayStatus::Active | RelayStatus::Connecting) {
                return Err("Relay is already running".to_string());
            }
            build_rtmp_url(&relay.rtmp_url, &relay.stream_key)
        };

        // Fetch pipeline handles (cloned GLib refs — cheap).
        let (pipeline, videotee, audiotee) = {
            let pipelines = self.pipelines.read().await;
            let sp = pipelines
                .get(&stream_id)
                .ok_or("No active pipeline for that stream")?;
            (
                sp.pipeline.clone(),
                sp.videotee.clone(),
                sp.audiotee.clone(),
            )
        };

        // Do GStreamer work on a blocking thread so we don't stall the async executor.
        let relay_elems =
            tokio::task::spawn_blocking(move || {
                attach_relay_to_pipeline(&pipeline, &videotee, &audiotee, &target_url)
            })
            .await
            .map_err(|e| format!("spawn_blocking failed: {}", e))?
            .map_err(|e| format!("GStreamer relay attach failed: {}", e))?;

        // Persist the relay element handles for later cleanup.
        {
            let mut relay_elements = self.relay_elements.write().await;
            relay_elements.insert(relay_id, relay_elems);
        }

        // Flip relay status to Active.
        {
            let mut relays = self.relays.write().await;
            let relay = relays.get_mut(&relay_id).ok_or("Relay not found")?;
            relay.stream_id = Some(stream_id);
            relay.status = RelayStatus::Active;
            relay.started_at = Some(Utc::now());
        }

        // Spawn health-monitor / auto-reconnect task.
        let state_clone = self.clone();
        tokio::spawn(monitor_relay(state_clone, relay_id, stream_id));

        Ok(())
    }

    pub async fn stop_relay(&self, relay_id: Uuid) -> Result<(), String> {
        // Look up which stream this relay was attached to.
        let stream_id = {
            let relays = self.relays.read().await;
            relays.get(&relay_id).ok_or("Relay not found")?;
            relays.get(&relay_id).and_then(|r| r.stream_id)
        };

        // Extract relay elements (takes ownership for cleanup).
        let relay_elems = {
            let mut relay_elements = self.relay_elements.write().await;
            relay_elements.remove(&relay_id)
        };

        // Detach from pipeline if we have both elements and a live pipeline.
        if let (Some(relay_elems), Some(stream_id)) = (relay_elems, stream_id) {
            let pipeline = {
                let pipelines = self.pipelines.read().await;
                pipelines.get(&stream_id).map(|sp| sp.pipeline.clone())
            };

            if let Some(pipeline) = pipeline {
                // Best-effort: log errors but don't fail the stop operation.
                if let Err(e) = tokio::task::spawn_blocking(move || {
                    detach_relay_from_pipeline(relay_elems, &pipeline)
                })
                .await
                {
                    tracing::warn!(relay_id = %relay_id, error = ?e, "Error detaching relay from pipeline");
                }
            }
        }

        // Update relay status regardless of GStreamer outcome.
        {
            let mut relays = self.relays.write().await;
            let relay = relays.get_mut(&relay_id).ok_or("Relay not found")?;
            relay.status = RelayStatus::Stopped;
            relay.stopped_at = Some(Utc::now());
            relay.stream_id = None;
        }

        Ok(())
    }

    pub async fn delete_relay(&self, relay_id: Uuid) -> Result<(), String> {
        // Stop first if running.
        {
            let relays = self.relays.read().await;
            if let Some(relay) = relays.get(&relay_id) {
                if matches!(relay.status, RelayStatus::Active | RelayStatus::Connecting) {
                    drop(relays);
                    self.stop_relay(relay_id).await?;
                }
            }
        }

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

    pub async fn add_log(&self, entry: LogEntry) {
        let mut logs = self.logs.write().await;
        if logs.len() >= 1000 {
            logs.remove(0);
        }
        logs.push(entry);
    }

    pub async fn get_logs(&self, limit: Option<usize>) -> Vec<LogEntry> {
        let logs = self.logs.read().await;
        let limit = limit.unwrap_or(100).min(1000);
        let start = logs.len().saturating_sub(limit);
        logs[start..].to_vec()
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Relay health monitor (async, runs as a spawned task)
// ---------------------------------------------------------------------------

async fn monitor_relay(state: AppState, relay_id: Uuid, stream_id: Uuid) {
    let mut reconnect_count = 0u32;
    let mut async_polls = 0u32;   // consecutive polls where rtmpsink was still Async
    let mut logged_playing = false;

    loop {
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;

        let relay_status = {
            let relays = state.relays.read().await;
            relays.get(&relay_id).map(|r| r.status.clone())
        };

        match relay_status {
            // Relay was manually stopped, deleted, or already in error — exit.
            None
            | Some(RelayStatus::Stopped)
            | Some(RelayStatus::Idle)
            | Some(RelayStatus::Error(_)) => break,
            // Still starting up — wait for next poll.
            Some(RelayStatus::Connecting) => continue,
            Some(RelayStatus::Active) => {
                let relay_info = {
                    let relay_elements = state.relay_elements.read().await;
                    relay_elements
                        .get(&relay_id)
                        .map(|e| (e.rtmpsink.clone(), e.bytes_sent.clone()))
                };
                let Some((rtmpsink, bytes_sent_atomic)) = relay_info else { break };

                // Sync bytes_sent counter to the relay record.
                {
                    let bytes = bytes_sent_atomic.load(Ordering::Relaxed);
                    let mut relays = state.relays.write().await;
                    if let Some(relay) = relays.get_mut(&relay_id) {
                        relay.bytes_sent = bytes;
                    }
                }

                // Check if rtmpsink is still in PLAYING (non-blocking, ZERO timeout).
                // StateChangeSuccess::Async means the element is still transitioning
                // (e.g. establishing the RTMP connection) — give it up to ~30 s.
                let (change_ret, gst_state, _) = rtmpsink.state(gstreamer::ClockTime::ZERO);
                if gst_state == gstreamer::State::Playing {
                    if !logged_playing {
                        tracing::info!(relay_id = %relay_id, "Relay reached PLAYING — data flowing");
                        logged_playing = true;
                    }
                    reconnect_count = 0;
                    async_polls = 0;
                    continue;
                }
                if matches!(change_ret, Ok(gstreamer::StateChangeSuccess::Async)) {
                    async_polls += 1;
                    if async_polls < 10 {
                        // Still connecting; give it more time (3 s × 10 = 30 s grace).
                        continue;
                    }
                    tracing::warn!(relay_id = %relay_id, "Relay stuck in Async after 30 s, treating as failure");
                }
                async_polls = 0;
                logged_playing = false;

                // rtmpsink left PLAYING — connection issue.
                let (auto_reconnect, delay_secs, max_attempts) = {
                    let config = state.config.read().await;
                    (
                        config.relay_auto_reconnect,
                        config.relay_reconnect_delay_secs,
                        config.relay_reconnect_attempts,
                    )
                };

                reconnect_count += 1;

                if auto_reconnect && reconnect_count <= max_attempts {
                    tracing::warn!(
                        relay_id = %relay_id,
                        attempt = reconnect_count,
                        max_attempts,
                        "Relay connection lost, reconnecting"
                    );
                    state
                        .add_log(LogEntry {
                            id: Uuid::new_v4().to_string(),
                            timestamp: chrono::Utc::now().to_rfc3339(),
                            level: LogLevel::Warn,
                            message: format!(
                                "Relay connection lost, reconnecting ({}/{})",
                                reconnect_count, max_attempts
                            ),
                            source: "relay".to_string(),
                        })
                        .await;

                    // Detach broken elements directly (avoids calling start_relay recursively,
                    // which would create a non-Send future cycle).
                    let old_elems = {
                        let mut relay_elements = state.relay_elements.write().await;
                        relay_elements.remove(&relay_id)
                    };
                    if let Some(old_elems) = old_elems {
                        let pipeline = {
                            let pipelines = state.pipelines.read().await;
                            pipelines.get(&stream_id).map(|sp| sp.pipeline.clone())
                        };
                        if let Some(pipeline) = pipeline {
                            tokio::task::spawn_blocking(move || {
                                detach_relay_from_pipeline(old_elems, &pipeline)
                            })
                            .await
                            .ok();
                        }
                    }

                    tokio::time::sleep(std::time::Duration::from_secs(delay_secs)).await;

                    // Check relay wasn't manually stopped while we slept.
                    let target_url = {
                        let relays = state.relays.read().await;
                        match relays.get(&relay_id) {
                            Some(r) if !matches!(r.status, RelayStatus::Active | RelayStatus::Connecting) => {
                                break; // manually stopped
                            }
                            Some(r) => build_rtmp_url(&r.rtmp_url, &r.stream_key),
                            None => break,
                        }
                    };

                    let pipeline_handles = {
                        let pipelines = state.pipelines.read().await;
                        pipelines.get(&stream_id).map(|sp| {
                            (sp.pipeline.clone(), sp.videotee.clone(), sp.audiotee.clone())
                        })
                    };

                    let Some((pipeline, videotee, audiotee)) = pipeline_handles else {
                        let mut relays = state.relays.write().await;
                        if let Some(r) = relays.get_mut(&relay_id) {
                            r.status = RelayStatus::Error("Stream ended during reconnect".to_string());
                        }
                        break;
                    };

                    let attach_result = tokio::task::spawn_blocking(move || {
                        attach_relay_to_pipeline(&pipeline, &videotee, &audiotee, &target_url)
                    })
                    .await
                    .map_err(|e| anyhow::anyhow!("spawn_blocking panic: {:?}", e))
                    .and_then(|r| r);

                    match attach_result {
                        Ok(new_elems) => {
                            {
                                let mut relay_elements = state.relay_elements.write().await;
                                relay_elements.insert(relay_id, new_elems);
                            }
                            {
                                let mut relays = state.relays.write().await;
                                if let Some(r) = relays.get_mut(&relay_id) {
                                    r.status = RelayStatus::Active;
                                    r.stream_id = Some(stream_id);
                                    r.started_at = Some(Utc::now());
                                    r.bytes_sent = 0;
                                }
                            }
                            async_polls = 0;
                            logged_playing = false;
                            tracing::info!(relay_id = %relay_id, "Relay reconnected successfully");
                            // Continue monitoring in this same loop — no new task needed.
                        }
                        Err(e) => {
                            tracing::error!(relay_id = %relay_id, error = %e, "Relay reconnect failed");
                            state
                                .add_log(LogEntry {
                                    id: Uuid::new_v4().to_string(),
                                    timestamp: chrono::Utc::now().to_rfc3339(),
                                    level: LogLevel::Error,
                                    message: format!("Relay reconnect failed: {}", e),
                                    source: "relay".to_string(),
                                })
                                .await;
                            let mut relays = state.relays.write().await;
                            if let Some(r) = relays.get_mut(&relay_id) {
                                r.status = RelayStatus::Error(format!("Reconnect failed: {}", e));
                            }
                            break;
                        }
                    }
                } else {
                    tracing::warn!(relay_id = %relay_id, reconnect_count, "Relay permanently lost");
                    state
                        .add_log(LogEntry {
                            id: Uuid::new_v4().to_string(),
                            timestamp: chrono::Utc::now().to_rfc3339(),
                            level: LogLevel::Error,
                            message: if auto_reconnect {
                                format!(
                                    "Relay connection permanently lost after {} reconnect attempts",
                                    reconnect_count.saturating_sub(1)
                                )
                            } else {
                                "Relay connection lost (auto-reconnect disabled)".to_string()
                            },
                            source: "relay".to_string(),
                        })
                        .await;
                    let mut relays = state.relays.write().await;
                    if let Some(r) = relays.get_mut(&relay_id) {
                        r.status = RelayStatus::Error("Connection lost".to_string());
                    }
                    break;
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn build_rtmp_url(rtmp_url: &str, stream_key: &str) -> String {
    if stream_key.is_empty() {
        rtmp_url.to_string()
    } else {
        format!("{}/{}", rtmp_url.trim_end_matches('/'), stream_key)
    }
}

// ---------------------------------------------------------------------------
// GStreamer helpers (called from spawn_blocking)
// ---------------------------------------------------------------------------

fn attach_relay_to_pipeline(
    pipeline: &gstreamer::Pipeline,
    videotee: &gstreamer::Element,
    audiotee: &gstreamer::Element,
    target_url: &str,
) -> anyhow::Result<RelayElements> {
    // Check required plugins are available before attempting to build.
    for plugin in ["queue", "h264parse", "aacparse", "flvmux", "rtmpsink"] {
        if gstreamer::ElementFactory::find(plugin).is_none() {
            anyhow::bail!(
                "GStreamer plugin '{}' not found. \
                 Install gst-plugins-good (flvmux, aacparse) and gst-plugins-bad (rtmpsink).",
                plugin
            );
        }
    }

    let queue_video = gstreamer::ElementFactory::make("queue").build()?;
    let queue_audio = gstreamer::ElementFactory::make("queue").build()?;
    // Explicit parsers ensure correct caps negotiation with flvmux.
    let h264parse_relay = gstreamer::ElementFactory::make("h264parse")
        .property("config-interval", -1i32)
        .build()?;
    let aacparse_relay = gstreamer::ElementFactory::make("aacparse").build()?;
    let flvmux = gstreamer::ElementFactory::make("flvmux")
        .property("streamable", true)
        .build()?;
    let rtmpsink = gstreamer::ElementFactory::make("rtmpsink")
        .property("location", target_url)
        .build()?;

    // Add all relay elements to the pipeline.
    pipeline.add_many([&queue_video, &queue_audio, &h264parse_relay, &aacparse_relay, &flvmux, &rtmpsink])?;

    // Wire relay-internal chain:  queues → parsers → flvmux → rtmpsink
    queue_video.link(&h264parse_relay)?;
    queue_audio.link(&aacparse_relay)?;

    let flvmux_video_pad = flvmux
        .request_pad_simple("video")
        .ok_or_else(|| anyhow::anyhow!("flvmux has no 'video' request pad"))?;
    let flvmux_audio_pad = flvmux
        .request_pad_simple("audio")
        .ok_or_else(|| anyhow::anyhow!("flvmux has no 'audio' request pad"))?;

    h264parse_relay
        .static_pad("src")
        .ok_or_else(|| anyhow::anyhow!("h264parse_relay has no src pad"))?
        .link(&flvmux_video_pad)?;
    aacparse_relay
        .static_pad("src")
        .ok_or_else(|| anyhow::anyhow!("aacparse_relay has no src pad"))?
        .link(&flvmux_audio_pad)?;
    flvmux.link(&rtmpsink)?;

    // Tap new src pads from the tees.
    let videotee_src_pad = videotee
        .request_pad_simple("src_%u")
        .ok_or_else(|| anyhow::anyhow!("Failed to request videotee src pad"))?;
    let audiotee_src_pad = audiotee
        .request_pad_simple("src_%u")
        .ok_or_else(|| anyhow::anyhow!("Failed to request audiotee src pad"))?;

    let queue_video_sink = queue_video
        .static_pad("sink")
        .ok_or_else(|| anyhow::anyhow!("queue_video has no sink pad"))?;
    let queue_audio_sink = queue_audio
        .static_pad("sink")
        .ok_or_else(|| anyhow::anyhow!("queue_audio has no sink pad"))?;

    videotee_src_pad.link(&queue_video_sink)?;
    audiotee_src_pad.link(&queue_audio_sink)?;

    // Probe on flvmux output to count bytes flowing to rtmpsink.
    let bytes_sent = Arc::new(AtomicU64::new(0));
    if let Some(flvmux_src) = flvmux.static_pad("src") {
        let bs = bytes_sent.clone();
        flvmux_src.add_probe(
            gstreamer::PadProbeType::BUFFER,
            move |_pad: &gstreamer::Pad, info: &mut gstreamer::PadProbeInfo<'_>| {
                if let Some(buf) = info.buffer() {
                    bs.fetch_add(buf.size() as u64, Ordering::Relaxed);
                }
                gstreamer::PadProbeReturn::Ok
            },
        );
    }

    // Bring all relay elements up to PLAYING to match the parent pipeline.
    for elem in [&queue_video, &queue_audio, &h264parse_relay, &aacparse_relay, &flvmux, &rtmpsink] {
        elem.sync_state_with_parent()?;
    }

    tracing::info!("Relay attached to pipeline");

    Ok(RelayElements {
        videotee: videotee.clone(),
        audiotee: audiotee.clone(),
        videotee_src_pad,
        audiotee_src_pad,
        queue_video,
        queue_audio,
        h264parse_relay,
        aacparse_relay,
        flvmux,
        rtmpsink,
        bytes_sent,
    })
}

fn detach_relay_from_pipeline(
    relay_elems: RelayElements,
    pipeline: &gstreamer::Pipeline,
) -> anyhow::Result<()> {
    // Unlink tee → queue connections first to stop data flowing into the relay.
    let queue_video_sink = relay_elems.queue_video.static_pad("sink");
    let queue_audio_sink = relay_elems.queue_audio.static_pad("sink");

    if let Some(sink) = queue_video_sink {
        relay_elems.videotee_src_pad.unlink(&sink).ok();
    }
    if let Some(sink) = queue_audio_sink {
        relay_elems.audiotee_src_pad.unlink(&sink).ok();
    }

    // Release the tee request pads.
    relay_elems
        .videotee
        .release_request_pad(&relay_elems.videotee_src_pad);
    relay_elems
        .audiotee
        .release_request_pad(&relay_elems.audiotee_src_pad);

    // Shut down relay elements from sink to source so data doesn't pile up.
    for elem in [
        &relay_elems.rtmpsink,
        &relay_elems.flvmux,
        &relay_elems.h264parse_relay,
        &relay_elems.aacparse_relay,
        &relay_elems.queue_video,
        &relay_elems.queue_audio,
    ] {
        let _ = elem.set_state(gstreamer::State::Null);
        let _ = pipeline.remove(elem);
    }

    tracing::info!("Relay detached from pipeline");

    Ok(())
}

// ---------------------------------------------------------------------------
// WebRTC helpers
// ---------------------------------------------------------------------------

/// Wires a `webrtcbin` branch onto the stream's tees, negotiates with the
/// browser's SDP offer, and signals `answer_tx` when ICE gathering is done.
/// Must be called from a blocking thread (`tokio::task::spawn_blocking`).
/// Finds the RTP payload type number the offering peer used for a given codec on a
/// given media type (e.g. "video"/"H264" or "audio"/"opus"), by scanning `a=rtpmap`
/// attributes. Offer/answer negotiation requires the answer to use one of the
/// payload type numbers the offer actually proposed for that codec - answering
/// with an arbitrary fixed number the offer never mentioned makes the whole
/// connection fail rather than just that codec.
fn find_offered_payload_type(
    sdp: &gstreamer_sdp::SDPMessage,
    media_type: &str,
    codec_name: &str,
) -> Option<u32> {
    for i in 0..sdp.medias_len() {
        let media = sdp.media(i)?;
        if media.media() != Some(media_type) {
            continue;
        }
        for attr in media.attributes() {
            if attr.key() != "rtpmap" {
                continue;
            }
            if let Some(value) = attr.value() {
                let mut parts = value.splitn(2, ' ');
                if let (Some(pt_str), Some(rest)) = (parts.next(), parts.next()) {
                    let encoding = rest.split('/').next().unwrap_or("");
                    if encoding.eq_ignore_ascii_case(codec_name) {
                        if let Ok(pt) = pt_str.parse::<u32>() {
                            return Some(pt);
                        }
                    }
                }
            }
        }
    }
    None
}

/// Rewrites an SDP so each video/audio media description lists only the one payload
/// type we actually negotiated, removing every other payload's format-list entry and
/// its associated a=rtpmap/a=fmtp/a=rtcp-fb attributes. See call site for why this
/// matters for real browsers.
fn narrow_answer_to_negotiated_payload_types(
    sdp: &mut gstreamer_sdp::SDPMessageRef,
    video_pt: u32,
    audio_pt: u32,
) {
    for i in 0..sdp.medias_len() {
        let Some(media) = sdp.media_mut(i) else { continue };
        let keep_pt = match media.media() {
            Some("video") => video_pt,
            Some("audio") => audio_pt,
            _ => continue,
        };
        narrow_media_to_payload_type(media, keep_pt);
    }
}

fn narrow_media_to_payload_type(media: &mut gstreamer_sdp::SDPMediaRef, keep_pt: u32) {
    // Drop every format-list entry (the payload numbers on the m= line) except the
    // one we're keeping. Iterate in reverse so earlier indices stay valid as we remove.
    for i in (0..media.formats_len()).rev() {
        if let Some(fmt) = media.format(i) {
            if fmt.parse::<u32>().ok() != Some(keep_pt) {
                let _ = media.remove_format(i);
            }
        }
    }

    // Drop rtpmap/fmtp/rtcp-fb attributes for every other payload type. Their value
    // always starts with "<payload-type> ...".
    for i in (0..media.attributes_len()).rev() {
        let Some(attr) = media.attribute(i) else { continue };
        if !matches!(attr.key(), "rtpmap" | "fmtp" | "rtcp-fb") {
            continue;
        }
        let Some(value) = attr.value() else { continue };
        let Some(pt_str) = value.split_whitespace().next() else { continue };
        if pt_str.parse::<u32>().ok() != Some(keep_pt) {
            let _ = media.remove_attribute(i);
        }
    }
}

pub fn attach_webrtc_to_pipeline(
    pipeline: &gstreamer::Pipeline,
    videotee: &gstreamer::Element,
    audiotee: &gstreamer::Element,
    stream_id: Uuid,
    offer_sdp: &str,
    stun_server: &str,
    answer_tx: Arc<Mutex<Option<tokio::sync::oneshot::Sender<anyhow::Result<String>>>>>,
) -> anyhow::Result<WebRTCSession> {
    use gstreamer_webrtc::{
        WebRTCBundlePolicy, WebRTCICEGatheringState, WebRTCSDPType, WebRTCSessionDescription,
    };
    use gstreamer_sdp::SDPMessage;

    // Fail fast if required plugins are absent.
    for plugin in [
        "queue", "h264parse", "rtph264pay",
        "audioconvert", "audioresample", "opusenc", "rtpopuspay",
        "webrtcbin",
    ] {
        if gstreamer::ElementFactory::find(plugin).is_none() {
            anyhow::bail!(
                "GStreamer plugin '{}' not found. \
                 Install gst-plugins-bad (webrtcbin, opusenc) and gst-plugins-good.",
                plugin
            );
        }
    }

    // Prefer avdec_aac (gst-libav) for AAC→PCM, fall back to faad.
    let aac_decoder_name = if gstreamer::ElementFactory::find("avdec_aac").is_some() {
        "avdec_aac"
    } else if gstreamer::ElementFactory::find("faad").is_some() {
        "faad"
    } else {
        anyhow::bail!(
            "No AAC decoder found. Install gst-libav (avdec_aac) or gst-plugins-bad (faad)."
        );
    };

    // Parse the offer up front so we can pay our RTP with whatever payload type
    // numbers the offering peer actually proposed for H264/OPUS. Browsers pick
    // their own numbering (Chrome commonly does NOT use 96/111), and an answer
    // that pays with a number the offer never proposed for that codec makes the
    // whole ICE/DTLS transport fail outright - not just a dropped track.
    let sdp_msg = SDPMessage::parse_buffer(offer_sdp.as_bytes())
        .map_err(|_| anyhow::anyhow!("Failed to parse offer SDP"))?;
    let video_pt = find_offered_payload_type(&sdp_msg, "video", "H264").unwrap_or(96);
    let audio_pt = find_offered_payload_type(&sdp_msg, "audio", "opus").unwrap_or(111);
    tracing::debug!(video_pt, audio_pt, "Negotiating WebRTC RTP payload types from offer");

    // --- Build elements ---
    let queue_v  = gstreamer::ElementFactory::make("queue").build()?;
    let h264parse = gstreamer::ElementFactory::make("h264parse")
        .property("config-interval", -1i32)  // inject SPS/PPS before every IDR
        .build()?;
    let rtph264pay = gstreamer::ElementFactory::make("rtph264pay")
        .property("pt", video_pt)
        .build()?;

    let queue_a      = gstreamer::ElementFactory::make("queue").build()?;
    let avdec_aac    = gstreamer::ElementFactory::make(aac_decoder_name).build()?;
    let audioconvert = gstreamer::ElementFactory::make("audioconvert").build()?;
    let audioresample = gstreamer::ElementFactory::make("audioresample").build()?;
    let opusenc      = gstreamer::ElementFactory::make("opusenc").build()?;
    let rtpopuspay   = gstreamer::ElementFactory::make("rtpopuspay")
        .property("pt", audio_pt)
        .build()?;

    let webrtcbin = gstreamer::ElementFactory::make("webrtcbin")
        .property("bundle-policy", WebRTCBundlePolicy::MaxBundle)
        .property("stun-server", format!("stun://{}", stun_server))
        .build()?;

    // All elements we'll add to the pipeline (for bulk removal later).
    let elements: Vec<gstreamer::Element> = vec![
        queue_v.clone(), h264parse.clone(), rtph264pay.clone(),
        queue_a.clone(), avdec_aac.clone(), audioconvert.clone(),
        audioresample.clone(), opusenc.clone(), rtpopuspay.clone(),
        webrtcbin.clone(),
    ];

    for elem in &elements {
        pipeline.add(elem)?;
    }

    // --- Wire video chain: queue_v → h264parse → rtph264pay → webrtcbin ---
    queue_v.link(&h264parse)?;
    h264parse.link(&rtph264pay)?;
    let webrtcbin_video_sink = webrtcbin
        .request_pad_simple("sink_%u")
        .ok_or_else(|| anyhow::anyhow!("webrtcbin: no video sink pad"))?;
    rtph264pay
        .static_pad("src")
        .ok_or_else(|| anyhow::anyhow!("rtph264pay: no src pad"))?
        .link(&webrtcbin_video_sink)?;

    // --- Wire audio chain: queue_a → avdec_aac → convert → resample → opusenc → rtpopuspay → webrtcbin ---
    queue_a.link(&avdec_aac)?;
    avdec_aac.link(&audioconvert)?;
    audioconvert.link(&audioresample)?;
    audioresample.link(&opusenc)?;
    opusenc.link(&rtpopuspay)?;
    let webrtcbin_audio_sink = webrtcbin
        .request_pad_simple("sink_%u")
        .ok_or_else(|| anyhow::anyhow!("webrtcbin: no audio sink pad"))?;
    rtpopuspay
        .static_pad("src")
        .ok_or_else(|| anyhow::anyhow!("rtpopuspay: no src pad"))?
        .link(&webrtcbin_audio_sink)?;

    // --- Tap from tees ---
    let vtee_src = videotee
        .request_pad_simple("src_%u")
        .ok_or_else(|| anyhow::anyhow!("videotee: no src pad"))?;
    let atee_src = audiotee
        .request_pad_simple("src_%u")
        .ok_or_else(|| anyhow::anyhow!("audiotee: no src pad"))?;

    vtee_src.link(
        &queue_v.static_pad("sink").ok_or_else(|| anyhow::anyhow!("queue_v: no sink"))?,
    )?;
    atee_src.link(
        &queue_a.static_pad("sink").ok_or_else(|| anyhow::anyhow!("queue_a: no sink"))?,
    )?;

    // --- ICE gathering complete handler (set up BEFORE remote description) ---
    let answer_tx_ice = answer_tx.clone();
    webrtcbin.connect_notify(Some("ice-gathering-state"), move |wb, _| {
        let state: WebRTCICEGatheringState = wb.property("ice-gathering-state");
        if state != WebRTCICEGatheringState::Complete {
            return;
        }
        let result = wb
            .property::<Option<WebRTCSessionDescription>>("local-description")
            .and_then(|d| d.sdp().as_text().ok())
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow::anyhow!("No local description after ICE complete"));

        if let Some(tx) = answer_tx_ice.lock().unwrap().take() {
            tx.send(result).ok();
        }
    });

    // --- Sync all elements to PLAYING BEFORE SDP exchange ---
    // webrtcbin must be at least in READY/PLAYING state before set-remote-description
    // and create-answer can succeed. Calling them on a NULL-state element silently fails.
    for elem in &elements {
        elem.sync_state_with_parent()?;
    }

    // --- Set remote description (browser's offer) ---
    let offer_desc = WebRTCSessionDescription::new(WebRTCSDPType::Offer, sdp_msg);

    let (srd_tx, srd_rx) = std::sync::mpsc::channel::<()>();
    let promise = gstreamer::Promise::with_change_func(move |_| { srd_tx.send(()).ok(); });
    webrtcbin.emit_by_name::<()>("set-remote-description", &[&offer_desc, &promise]);
    srd_rx.recv().ok();

    // --- Create answer then set-local-description from THIS thread (avoids deadlock).
    //     Calling set-local-description inside the create-answer callback can deadlock
    //     if GStreamer resolves that promise on the same thread the callback is running on.
    let wb_clone = webrtcbin.clone();
    let (ca_tx, ca_rx) = std::sync::mpsc::channel::<anyhow::Result<WebRTCSessionDescription>>();
    let promise = gstreamer::Promise::with_change_func(move |reply| {
        let result: anyhow::Result<WebRTCSessionDescription> = (|| {
            let reply = reply
                .map_err(|e| anyhow::anyhow!("create-answer error: {:?}", e))?
                .ok_or_else(|| anyhow::anyhow!("create-answer: empty reply"))?;
            let answer: WebRTCSessionDescription = reply
                .value("answer")
                .map_err(|e| anyhow::anyhow!("no answer field: {:?}", e))?
                .get()
                .map_err(|e| anyhow::anyhow!("wrong type: {:?}", e))?;
            Ok(answer)
        })();
        ca_tx.send(result).ok();
    });
    webrtcbin.emit_by_name::<()>("create-answer", &[&None::<gstreamer::Structure>, &promise]);

    let mut answer = ca_rx
        .recv()
        .map_err(|e| anyhow::anyhow!("create-answer channel error: {}", e))??;

    // webrtcbin's create-answer copies every payload-type variant the offer proposed
    // for a codec family we support (e.g. Chrome offers H264 at 5+ different payload
    // types for different profiles), rather than narrowing to the single one we
    // actually send. Real browsers enforce strict per-transceiver payload matching
    // (unlike GStreamer's own rtpbin, which is lenient) - if the browser picks a
    // payload type from that list other than the one our rtph264pay/rtpopuspay
    // actually emit, it silently discards every packet: ICE/DTLS connect fine, no
    // error fires, the video is just never decodable. Narrow the answer down to
    // exactly the payload types we negotiated before it goes out.
    narrow_answer_to_negotiated_payload_types(answer.sdp_mut(), video_pt, audio_pt);

    // Set local description from this spawn_blocking thread — safe.
    let (sld_tx, sld_rx) = std::sync::mpsc::channel::<()>();
    let p = gstreamer::Promise::with_change_func(move |_| { sld_tx.send(()).ok(); });
    wb_clone.emit_by_name::<()>("set-local-description", &[&answer, &p]);
    sld_rx.recv().ok();

    // Guard against a race where ICE gathering completed synchronously.
    let ice_state: WebRTCICEGatheringState = webrtcbin.property("ice-gathering-state");
    if ice_state == WebRTCICEGatheringState::Complete {
        let result = webrtcbin
            .property::<Option<WebRTCSessionDescription>>("local-description")
            .and_then(|d| d.sdp().as_text().ok())
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow::anyhow!("No local description"));
        if let Some(tx) = answer_tx.lock().unwrap().take() {
            tx.send(result).ok();
        }
    }

    tracing::info!(stream_id = %stream_id, "WebRTC session attached to pipeline");

    Ok(WebRTCSession {
        stream_id,
        pipeline: pipeline.clone(),
        videotee: videotee.clone(),
        audiotee: audiotee.clone(),
        vtee_src_pad: vtee_src,
        atee_src_pad: atee_src,
        elements,
    })
}

fn detach_webrtc_from_pipeline(session: WebRTCSession) {
    // Unlink tee → queue_v / queue_a
    if let Some(sink) = session.elements.first().and_then(|e| e.static_pad("sink")) {
        session.vtee_src_pad.unlink(&sink).ok();
    }
    if let (Some(qa), _) = (session.elements.get(3), ()) {
        if let Some(sink) = qa.static_pad("sink") {
            session.atee_src_pad.unlink(&sink).ok();
        }
    }
    session.videotee.release_request_pad(&session.vtee_src_pad);
    session.audiotee.release_request_pad(&session.atee_src_pad);

    // Set to NULL and remove from pipeline (reverse order: sink first).
    for elem in session.elements.iter().rev() {
        let _ = elem.set_state(gstreamer::State::Null);
        let _ = session.pipeline.remove(elem);
    }

    tracing::info!(stream_id = %session.stream_id, "WebRTC session detached");
}

#[cfg(test)]
mod tests {
    use super::find_offered_payload_type;

    const BROWSER_LIKE_OFFER: &str = "v=0\r\n\
o=- 1 1 IN IP4 0.0.0.0\r\n\
s=-\r\n\
t=0 0\r\n\
a=group:BUNDLE video0 audio1\r\n\
m=video 9 UDP/TLS/RTP/SAVPF 102 96\r\n\
c=IN IP4 0.0.0.0\r\n\
a=mid:video0\r\n\
a=rtpmap:102 H264/90000\r\n\
a=fmtp:102 packetization-mode=1\r\n\
a=rtpmap:96 VP8/90000\r\n\
a=recvonly\r\n\
m=audio 9 UDP/TLS/RTP/SAVPF 109\r\n\
c=IN IP4 0.0.0.0\r\n\
a=mid:audio1\r\n\
a=rtpmap:109 opus/48000/2\r\n\
a=recvonly\r\n";

    #[test]
    fn finds_offered_h264_payload_type_even_when_not_first() {
        gstreamer::init().unwrap();
        let sdp = gstreamer_sdp::SDPMessage::parse_buffer(BROWSER_LIKE_OFFER.as_bytes()).unwrap();
        assert_eq!(find_offered_payload_type(&sdp, "video", "H264"), Some(102));
    }

    #[test]
    fn finds_offered_opus_payload_type_case_insensitively() {
        gstreamer::init().unwrap();
        let sdp = gstreamer_sdp::SDPMessage::parse_buffer(BROWSER_LIKE_OFFER.as_bytes()).unwrap();
        // SDP declares "opus" lowercase; our lookup is called with "opus" too, but
        // must also match e.g. "OPUS" from other encoders - verified via eq_ignore_ascii_case.
        assert_eq!(find_offered_payload_type(&sdp, "audio", "OPUS"), Some(109));
    }

    #[test]
    fn returns_none_for_codec_not_offered() {
        gstreamer::init().unwrap();
        let sdp = gstreamer_sdp::SDPMessage::parse_buffer(BROWSER_LIKE_OFFER.as_bytes()).unwrap();
        assert_eq!(find_offered_payload_type(&sdp, "video", "AV1"), None);
    }
}
