use crate::events::AppEvents;
use crate::rtmp::flv_parser::FlvTagParser;
use crate::rtmp::utils::{
    extract_flv_tag_payload, is_audio_aac_sequence_header, is_video_keyframe_avc_sequence_header,
    FlvTagType,
};
use bytes::Bytes;
use rml_rtmp::sessions::{ClientSession, ClientSessionResult};
use rml_rtmp::time::RtmpTimestamp;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncReadExt, AsyncWriteExt, WriteHalf};
use tokio::net::TcpStream;
use tokio::sync::{broadcast, Mutex};

const MAX_BUFFER_SIZE: usize = 150; // ~5 seconds at 30fps
const BUFFER_DROP_COUNT: usize = 50; // Drop oldest 50 frames when full

use std::collections::VecDeque;

/// Buffered writer with backpressure handling
pub struct BufferedRelayWriter {
    writer: Arc<Mutex<WriteHalf<TcpStream>>>,
    buffer: VecDeque<Bytes>,
    dropped_frames: u64,
}

impl BufferedRelayWriter {
    pub fn new(writer: Arc<Mutex<WriteHalf<TcpStream>>>) -> Self {
        Self {
            writer,
            buffer: VecDeque::with_capacity(MAX_BUFFER_SIZE),
            dropped_frames: 0,
        }
    }

    /// Add data to buffer with drop-oldest policy
    pub fn push(&mut self, data: Bytes) {
        if self.buffer.len() >= MAX_BUFFER_SIZE {
            let before = self.buffer.len();
            // Drop oldest frames to prevent memory bloat
            for _ in 0..BUFFER_DROP_COUNT.min(self.buffer.len()) {
                self.buffer.pop_front();
                self.dropped_frames += 1;
            }

            eprintln!(
                "⚠️ Relay buffer full: dropped {} frames (before: {}, after: {}, total dropped: {})",
                BUFFER_DROP_COUNT.min(before),
                before,
                self.buffer.len(),
                self.dropped_frames
            );
        }

        self.buffer.push_back(data);
    }

    /// Flush buffer to socket (non-blocking attempt)
    pub async fn flush(&mut self) -> Result<usize, std::io::Error> {
        let mut written = 0;
        let initial_buffer_len = self.buffer.len();
        let start_time = std::time::Instant::now();

        while let Some(data) = self.buffer.pop_front() {
            match tokio::time::timeout(std::time::Duration::from_millis(100), self.writer.lock())
                .await
            {
                Ok(mut guard) => {
                    match guard.write_all(&data).await {
                        Ok(_) => written += 1,
                        Err(e) => {
                            // Put back and return error
                            self.buffer.push_front(data);
                            eprintln!(
                                "❌ Relay flush write error after writing {}/{} packets: {} (kind: {:?})",
                                written, initial_buffer_len, e, e.kind()
                            );
                            return Err(e);
                        }
                    }
                }
                Err(_) => {
                    // Timeout acquiring lock, put back and continue later
                    self.buffer.push_front(data);
                    eprintln!(
                        "⚠️ Relay flush lock timeout after writing {}/{} packets in {:?}",
                        written,
                        initial_buffer_len,
                        start_time.elapsed()
                    );
                    break;
                }
            }
        }

        if written > 0 || initial_buffer_len > 10 {
            println!(
                "📤 Relay flushed {}/{} packets in {:?}, buffer remaining: {}",
                written,
                initial_buffer_len,
                start_time.elapsed(),
                self.buffer.len()
            );
        }

        Ok(written)
    }

    pub fn buffer_len(&self) -> usize {
        self.buffer.len()
    }
}

/// Start the pump task that sends data from encoder to relay server
pub fn start_pump_task(
    relay_id: i64,
    mut rx: broadcast::Receiver<Bytes>,
    session: Arc<Mutex<ClientSession>>,
    writer: Arc<Mutex<WriteHalf<TcpStream>>>,
    active: Arc<AtomicBool>,
    app_handle: AppHandle,
) -> tauri::async_runtime::JoinHandle<()> {
    tauri::async_runtime::spawn(async move {
        let mut flush_interval = tokio::time::interval(std::time::Duration::from_millis(50));
        let mut packets_processed = 0u64;
        let mut audio_packets = 0u64;
        let mut video_packets = 0u64;
        let mut bytes_sent = 0u64;
        let start_time = std::time::Instant::now();
        let mut last_log_time = start_time;
        let mut buffered_writer = BufferedRelayWriter::new(writer);

        println!("🚀 Relay {} pump task started", relay_id);

        loop {
            tokio::select! {
                // Receive data from encoder
                chunk = rx.recv() => {
                    match chunk {
                        Ok(data) => {
                            // Check if this is a sequence header (critical for stream initialization)
                            let is_video_seq_header = is_video_keyframe_avc_sequence_header(&data);
                            let is_audio_seq_header = is_audio_aac_sequence_header(&data);

                            if is_video_seq_header {
                                println!("🎬 Relay {} received video sequence header", relay_id);
                            } else if is_audio_seq_header {
                                println!("🎵 Relay {} received audio sequence header", relay_id);
                            }

                            if let Some((tag_type, timestamp, payload)) = extract_flv_tag_payload(&data) {
                                let payload_bytes = Bytes::from(payload);
                                let timestamp = RtmpTimestamp::new(timestamp);

                                let resp = match tag_type {
                                    FlvTagType::Audio => {
                                        audio_packets += 1;
                                        session
                                            .lock()
                                            .await
                                            .publish_audio_data(payload_bytes, timestamp, false)
                                            .ok()
                                    }
                                    FlvTagType::Video => {
                                        video_packets += 1;
                                        session
                                            .lock()
                                            .await
                                            .publish_video_data(payload_bytes, timestamp, false)
                                            .ok()
                                    }
                                    _ => None,
                                };

                                if let Some(ClientSessionResult::OutboundResponse(pkt)) = resp {
                                    let pkt_size = pkt.bytes.len() as u64;
                                    buffered_writer.push(Bytes::from(pkt.bytes));
                                    packets_processed += 1;
                                    bytes_sent += pkt_size;
                                }

                                // Log stats every 5 seconds
                                if last_log_time.elapsed() >= std::time::Duration::from_secs(5) {
                                    let uptime = start_time.elapsed();
                                    println!(
                                        "📊 Relay {} stats: uptime={:?}, packets={} (audio={}, video={}), bytes_sent={}, buffer={}",
                                        relay_id, uptime, packets_processed, audio_packets, video_packets, bytes_sent, buffered_writer.buffer_len()
                                    );
                                    last_log_time = std::time::Instant::now();
                                }
                            } else {
                                eprintln!("⚠️ Relay {} failed to extract FLV tag payload from {} bytes", relay_id, data.len());
                            }
                        }
                        Err(broadcast::error::RecvError::Lagged(skipped)) => {
                            eprintln!("⚠️ Relay {} lagged, skipped {} messages (total packets processed: {})", relay_id, skipped, packets_processed);
                            continue;
                        }
                        Err(e) => {
                            eprintln!("❌ Relay {} broadcast channel error: {:?} (after processing {} packets)", relay_id, e, packets_processed);
                            active.store(false, Ordering::SeqCst);
                            break;
                        }
                    }
                }

                // Periodic flush
                _ = flush_interval.tick() => {
                    let buffer_len = buffered_writer.buffer_len();
                    if buffer_len > 0 {
                        if let Err(e) = buffered_writer.flush().await {
                            eprintln!(
                                "❌ Relay {} flush error: {} (kind: {:?}, buffer: {}, packets processed: {})",
                                relay_id, e, e.kind(), buffer_len, packets_processed
                            );
                            active.store(false, Ordering::SeqCst);
                            let _ = app_handle.emit(AppEvents::RelayFailed.as_str(), relay_id);
                            break;
                        }
                    }
                }
            }
        }

        let uptime = start_time.elapsed();
        println!(
            "🛑 Relay {} pump task ended after {:?} ({} total packets: {} audio, {} video)",
            relay_id, uptime, packets_processed, audio_packets, video_packets
        );
    })
}

/// Start the reader task that handles incoming messages from relay server
pub fn start_reader_task(
    relay_id: i64,
    reader: Arc<Mutex<tokio::io::ReadHalf<TcpStream>>>,
    session: Arc<Mutex<ClientSession>>,
    writer: Arc<Mutex<WriteHalf<TcpStream>>>,
    active: Arc<AtomicBool>,
    app_handle: AppHandle,
) -> tauri::async_runtime::JoinHandle<()> {
    tauri::async_runtime::spawn(async move {
        let mut buf = [0u8; 4096];
        let mut read_count = 0u64;
        let mut ping_interval = tokio::time::interval(std::time::Duration::from_secs(30));

        println!("📖 Relay {} reader task started", relay_id);

        loop {
            tokio::select! {
                // Read from server
                result = async {
                    let mut guard = reader.lock().await;
                    guard.read(&mut buf).await
                } => {
                    match result {
                        Ok(0) => {
                            eprintln!("❌ Relay {} server closed connection (after {} reads)", relay_id, read_count);
                            active.store(false, Ordering::SeqCst);
                            let _ = app_handle.emit(AppEvents::RelayFailed.as_str(), relay_id);
                            break;
                        }
                        Ok(n) => {
                            read_count += 1;
                            println!("📥 Relay {} received {} bytes from server (read #{})", relay_id, n, read_count);

                            // Process incoming messages
                            match session.lock().await.handle_input(&buf[..n]) {
                                Ok(responses) => {
                                    for res in responses {
                                        match res {
                                            ClientSessionResult::RaisedEvent(event) => {
                                                // Only log non-acknowledgement events to reduce noise
                                                match &event {
                                                    rml_rtmp::sessions::ClientSessionEvent::AcknowledgementReceived { bytes_received } => {
                                                        // Log every 10th ack to reduce spam
                                                        if read_count % 10 == 0 {
                                                            println!("📢 Relay {} ACK received: {} bytes (read #{})", relay_id, bytes_received, read_count);
                                                        }
                                                    }
                                                    _ => {
                                                        println!("📢 Relay {} server event: {:?}", relay_id, event);
                                                    }
                                                }
                                            }
                                            ClientSessionResult::OutboundResponse(pkt) => {
                                                println!("📤 Relay {} sending response to server ({} bytes)", relay_id, pkt.bytes.len());
                                                if let Err(e) = writer.lock().await.write_all(&pkt.bytes).await {
                                                    eprintln!("❌ Relay {} failed to write response: {} (kind: {:?})", relay_id, e, e.kind());
                                                    active.store(false, Ordering::SeqCst);
                                                    let _ = app_handle.emit(AppEvents::RelayFailed.as_str(), relay_id);
                                                    break;
                                                }
                                            }
                                            ClientSessionResult::UnhandleableMessageReceived(payload) => {
                                                eprintln!("⚠️ Relay {} unhandled server message: {:?}", relay_id, payload);
                                            }
                                        }
                                    }
                                }
                                Err(e) => {
                                    eprintln!("❌ Relay {} error processing server input: {}", relay_id, e);
                                    active.store(false, Ordering::SeqCst);
                                    let _ = app_handle.emit(AppEvents::RelayFailed.as_str(), relay_id);
                                    break;
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("❌ Relay {} read error: {} (kind: {:?}, after {} reads)", relay_id, e, e.kind(), read_count);
                            active.store(false, Ordering::SeqCst);
                            let _ = app_handle.emit(AppEvents::RelayFailed.as_str(), relay_id);
                            break;
                        }
                    }
                }

                // Send periodic keepalive ping
                _ = ping_interval.tick() => {
                    if let Ok((pkt, _)) = session.lock().await.send_ping_request() {
                        if let Err(e) = writer.lock().await.write_all(&pkt.bytes).await {
                            eprintln!("❌ Relay {} failed to send keepalive ping: {} (kind: {:?})", relay_id, e, e.kind());
                            active.store(false, Ordering::SeqCst);
                            let _ = app_handle.emit(AppEvents::RelayFailed.as_str(), relay_id);
                            break;
                        } else {
                            println!("🏓 Relay {} sent keepalive ping", relay_id);
                        }
                    }
                }
            }
        }

        println!("🛑 Relay {} reader task ended", relay_id);
    })
}
