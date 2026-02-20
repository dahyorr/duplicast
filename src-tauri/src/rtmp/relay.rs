use crate::rtmp::handshake::handle_relay_handshake;
use crate::rtmp::utils::{extract_flv_tag_payload, FlvTagType};
use crate::{config, db, events::AppEvents, models};
use bytes::Bytes;
use rml_rtmp::sessions::{
    ClientSession, ClientSessionConfig, ClientSessionEvent, ClientSessionResult, PublishRequestType,
};
use rml_rtmp::time::RtmpTimestamp;
use std::collections::VecDeque;
use std::net::ToSocketAddrs;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::{split, AsyncReadExt};
use tokio::net::TcpStream;
use tokio::{
    io::{AsyncWriteExt, ReadHalf, WriteHalf},
    sync::{broadcast, Mutex},
};
use url::Url;

const MAX_BUFFER_SIZE: usize = 150; // ~5 seconds at 30fps
const BUFFER_DROP_COUNT: usize = 50; // Drop oldest 50 frames when full

#[derive(Debug)]
struct RelayCredentials {
    url: String,
    stream_key: String,
}

/// Buffered writer with backpressure handling
struct BufferedRelayWriter {
    writer: Arc<Mutex<WriteHalf<TcpStream>>>,
    buffer: VecDeque<Bytes>,
    dropped_frames: u64,
}

impl BufferedRelayWriter {
    fn new(writer: Arc<Mutex<WriteHalf<TcpStream>>>) -> Self {
        Self {
            writer,
            buffer: VecDeque::with_capacity(MAX_BUFFER_SIZE),
            dropped_frames: 0,
        }
    }

    /// Add data to buffer with drop-oldest policy
    fn push(&mut self, data: Bytes) {
        if self.buffer.len() >= MAX_BUFFER_SIZE {
            // Drop oldest frames to prevent memory bloat
            for _ in 0..BUFFER_DROP_COUNT.min(self.buffer.len()) {
                self.buffer.pop_front();
                self.dropped_frames += 1;
            }

            if self.dropped_frames % 100 == 0 {
                eprintln!(
                    "⚠️ Relay buffer full, dropped {} frames total",
                    self.dropped_frames
                );
            }
        }

        self.buffer.push_back(data);
    }

    /// Flush buffer to socket (non-blocking attempt)
    async fn flush(&mut self) -> Result<usize, std::io::Error> {
        let mut written = 0;

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
                            return Err(e);
                        }
                    }
                }
                Err(_) => {
                    // Timeout acquiring lock, put back and continue later
                    self.buffer.push_front(data);
                    break;
                }
            }
        }

        Ok(written)
    }

    fn buffer_len(&self) -> usize {
        self.buffer.len()
    }
}

pub struct RelayHandle {
    pub id: i64,
    pub active: Arc<AtomicBool>,
    credentials: RelayCredentials,
    pub rx: broadcast::Receiver<Bytes>,
    pub rx_task: Option<tauri::async_runtime::JoinHandle<()>>,
    pub reader: Option<Arc<Mutex<ReadHalf<TcpStream>>>>,
    pub writer: Option<Arc<Mutex<WriteHalf<TcpStream>>>>,
    pub session: Option<Arc<Mutex<ClientSession>>>,
}

impl RelayHandle {
    pub fn from_relay_target(
        relay: &models::RelayTarget,
        encoder_rx: broadcast::Receiver<Bytes>,
    ) -> Self {
        Self {
            id: relay.id,
            credentials: RelayCredentials {
                url: relay.url.clone(),
                stream_key: relay.stream_key.clone(),
            },
            active: Arc::new(AtomicBool::new(false)),
            rx: encoder_rx,
            rx_task: None,
            reader: None,
            writer: None,
            session: None,
        }
    }

    pub fn is_active(&self) -> bool {
        self.active.load(Ordering::SeqCst)
    }

    pub async fn start(
        &mut self,
        app: &AppHandle,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let url = Url::parse(&self.credentials.url)?;
        let host = url.host_str().ok_or("missing host")?;
        let port = url.port().unwrap_or(1935);
        // let mut path_segments = url.path_segments().ok_or("invalid path")?;
        // let app_name = path_segments.next().ok_or("missing app name")?.to_string();

        // TCP dial
        let addr = (host, port)
            .to_socket_addrs()?
            .next()
            .ok_or("cannot resolve")?;
        let socket = TcpStream::connect(addr).await?;
        socket.set_nodelay(true)?;

        // handshake & connect
        let (socket, remaining) = handle_relay_handshake(socket).await.map_err(
            |e| -> Box<dyn std::error::Error + Send + Sync> {
                format!("Handshake failed: {}", e).into()
            },
        )?;
        let (socket, session) = self.setup_rtmp_client_session(socket, remaining).await?;
        let (reader, writer) = split(socket);

        let reader = Arc::new(Mutex::new(reader));
        let writer = Arc::new(Mutex::new(writer));
        let session = Arc::new(Mutex::new(session));
        // self.socket = Some(socket.clone());
        self.session = Some(session.clone());
        self.writer = Some(writer.clone());
        self.reader = Some(reader.clone());

        // Process initial handshake and connection
        let mut buf = [0u8; 4096];
        let mut publish_accepted = false;

        // Read and process messages until publish is accepted
        while !publish_accepted {
            let n = match reader.lock().await.read(&mut buf).await {
                Ok(0) => return Err("Server closed connection before publish accepted".into()),
                Ok(n) => n,
                Err(e) => {
                    eprintln!("❌ Relay {} read error: {}", self.id, e);
                    return Err(format!("Read error: {}", e).into());
                }
            };
            // if n == 0 {
            //     return Err("RTMP server closed connection".into());
            // }
            let responses = session.lock().await.handle_input(&buf[..n])?;
            for res in responses {
                match res {
                    ClientSessionResult::RaisedEvent(event) => {                        // Check if publish was accepted to break the loop
                        if matches!(event, ClientSessionEvent::PublishRequestAccepted) {
                            publish_accepted = true;
                        }
                                                // now request connect to “app” (everything after the host in the URL)
                        // self.handle_relay_session_event(app, event).await?;
                        if let Err(e) = self.handle_relay_session_event(app, event).await {
                            eprintln!("⚠️ Relay session event error ({}): {}", self.id, e);
                            let reason = e.to_string();
                            self.handle_relay_failed(app, &reason);

                            // or continue, depending on your retry strategy
                        }
                    }
                    ClientSessionResult::OutboundResponse(pkt) => {
                        println!("writing Outbound Response");
                        writer.lock().await.write_all(&pkt.bytes).await?;
                        println!("DOne writing Outbound Response");
                    }
                    ClientSessionResult::UnhandleableMessageReceived(payload) => {
                        eprintln!("RTMP Unhandled: {:?}", payload);
                    }
                }
            }
        }
        Ok(())
    }

    pub async fn stop(&mut self, app: &AppHandle) {
        if self.active.load(Ordering::SeqCst) {
            // 1. Abort the pump task if it's running
            if let Some(task) = self.rx_task.take() {
                task.abort(); // Stops the background spawn
            }

            // 3. Clear socket/session (optional)
            self.writer = None;
            self.reader = None;
            self.session = None;

            // 4. Emit frontend event
            let _ = app.emit(AppEvents::RelayEnded.as_str(), self.id);

            // 5. Mark as inactive
            self.active.store(false, Ordering::SeqCst);
            println!("🔴 Relay {} stopped", self.id);
        } else {
            println!("Relay {} is already inactive", self.id);
        }
    }

    pub fn handle_relay_failed(&mut self, app: &AppHandle, reason: &str) {
        eprintln!("🔴 Relay {} failed: {}", self.id, reason);
        if let Some(task) = self.rx_task.take() {
            task.abort();
        }
        self.writer = None;
        self.reader = None;
        self.session = None;

        let _ = app.emit(AppEvents::RelayFailed.as_str(), self.id);
        self.active.store(false, Ordering::SeqCst);
    }

    async fn setup_rtmp_client_session(
        &mut self,
        mut socket: TcpStream,
        remaining: Vec<u8>,
    ) -> Result<(TcpStream, ClientSession), Box<dyn std::error::Error + Send + Sync>> {
        let url = Url::parse(&self.credentials.url)?;
        let mut path_segments = url.path_segments().ok_or("invalid path")?;
        let app_name = path_segments.next().ok_or("missing app name")?.to_string();

        let config = ClientSessionConfig::new();
        let (mut session, initial_session_results) = match ClientSession::new(config) {
            Ok(results) => results,
            Err(error) => return Err(error.to_string().into()),
        };
        if !remaining.is_empty() {
            let responses = session.handle_input(&remaining)?;
            for resp in responses {
                if let ClientSessionResult::OutboundResponse(pkt) = resp {
                    socket.write_all(&pkt.bytes).await?;
                }
            }
        }

        for result in initial_session_results {
            if let ClientSessionResult::OutboundResponse(packet) = result {
                socket.write_all(&packet.bytes).await?;
            }
        }

        let connect_req = session.request_connection(app_name)?;
        if let ClientSessionResult::OutboundResponse(pkt) = connect_req {
            socket.write_all(&pkt.bytes).await?;
        }

        Ok((socket, session))
    }

    async fn handle_relay_session_event(
        &mut self,
        app: &AppHandle,
        event: ClientSessionEvent,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let session = self.session.clone().unwrap();
        let writer = self.writer.clone().unwrap();
        match event {
            ClientSessionEvent::ConnectionRequestAccepted => {
                let r = session.lock().await.request_publishing(
                    self.credentials.stream_key.clone(),
                    PublishRequestType::Live,
                )?;
                if let ClientSessionResult::OutboundResponse(pkt) = r {
                    writer.lock().await.write_all(&pkt.bytes).await?;
                }
                println!("Relay:{} connect Request Accepted", self.id);
            }
            ClientSessionEvent::ConnectionRequestRejected { description } => {
                println!("Connection Failed: {description}");
            }
            ClientSessionEvent::PublishRequestAccepted => {
                println!("Relay:{} Publish Request Accepted", self.id);
                self.start_pump(app).await?;
            }
            ClientSessionEvent::AcknowledgementReceived {
                bytes_received: _bytes_received,
            } => {
                println!("ACK {_bytes_received}");
            }
            ClientSessionEvent::PingResponseReceived {
                timestamp: _timestamp,
            } => {
                println!("PING!!");
            }
            ev => {
                println!("Unknown event {:?}", ev);
            }
        }
        Ok(())
    }

    async fn start_pump(
        &mut self,
        app: &AppHandle,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let session = self.session.clone().unwrap();
        let writer = self.writer.clone().unwrap();
        let state = app.state::<Arc<config::AppState>>();
        let metadata = state.source_metadata.lock().await;
        let (ping_pkt, _) = session.lock().await.send_ping_request().unwrap();

        writer.lock().await.write_all(&ping_pkt.bytes).await?;
        println!("Ping Sent");

        if let Some(metadata) = metadata.as_ref() {
            println!("Sending Metadata: {:?}", metadata);
            let r = session.lock().await.publish_metadata(metadata)?;
            if let ClientSessionResult::OutboundResponse(pkt) = r {
                writer.lock().await.write_all(&pkt.bytes).await?;
                println!("Metadata Sent");
            }
        }

        let mut rx = self.rx.resubscribe();
        let relay_id = self.id;
        let active = self.active.clone();
        let app_handle = app.clone();

        // Create buffered writer for this relay
        let mut buffered_writer = BufferedRelayWriter::new(writer.clone());

        let task = tauri::async_runtime::spawn(async move {
            let mut flush_interval = tokio::time::interval(std::time::Duration::from_millis(50));

            loop {
                tokio::select! {
                    // Receive data from encoder
                    chunk = rx.recv() => {
                        match chunk {
                            Ok(data) => {
                                if let Some((tag_type, timestamp, payload)) = extract_flv_tag_payload(&data) {
                                    let payload_bytes = Bytes::from(payload);
                                    let timestamp = RtmpTimestamp::new(timestamp);

                                    let resp = match tag_type {
                                        FlvTagType::Audio => session
                                            .lock()
                                            .await
                                            .publish_audio_data(payload_bytes, timestamp, false)
                                            .ok(),
                                        FlvTagType::Video => session
                                            .lock()
                                            .await
                                            .publish_video_data(payload_bytes, timestamp, false)
                                            .ok(),
                                        _ => None,
                                    };

                                    if let Some(ClientSessionResult::OutboundResponse(pkt)) = resp {
                                        buffered_writer.push(Bytes::from(pkt.bytes));
                                    }
                                }
                            }
                            Err(broadcast::error::RecvError::Lagged(skipped)) => {
                                eprintln!("⚠️ Relay {} lagged, skipped {} messages", relay_id, skipped);
                                continue;
                            }
                            Err(_) => {
                                eprintln!("❌ Relay {} broadcast channel closed", relay_id);
                                active.store(false, std::sync::atomic::Ordering::SeqCst);
                                break;
                            }
                        }
                    }

                    // Periodic flush
                    _ = flush_interval.tick() => {
                        if buffered_writer.buffer_len() > 0 {
                            if let Err(e) = buffered_writer.flush().await {
                                eprintln!("❌ Relay {} flush error: {}", relay_id, e);
                                active.store(false, std::sync::atomic::Ordering::SeqCst);
                                let _ = app_handle.emit(AppEvents::RelayFailed.as_str(), relay_id);
                                break;
                            }
                        }
                    }
                }
            }

            println!("🛑 Relay {} pump task ended", relay_id);
        });

        self.rx_task = Some(task);

        // Notify the UI
        app.emit(AppEvents::RelayActive.as_str(), self.id)?;
        println!("🟢 Relay {} started", self.id);
        self.active.store(true, Ordering::SeqCst);
        Ok(())
    }
}

pub async fn start_relays(app: &AppHandle) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let pool = db::get_db_pool();
    let relays = db::get_active_relay_targets(&pool).await.map_err(
        |e| -> Box<dyn std::error::Error + Send + Sync> { format!("DB error: {}", e).into() },
    )?;
    let state = app.state::<Arc<config::AppState>>();
    let tx = state.encoder_tx.clone();

    for relay in relays {
        let mut relay_handle = RelayHandle::from_relay_target(&relay, tx.subscribe());
        relay_handle.start(app).await?;
        state.relays.lock().await.insert(relay.id, relay_handle);
    }
    Ok(())
}

pub async fn stop_relays(app: &AppHandle) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let state = app.state::<Arc<config::AppState>>();
    let mut relays = state.relays.lock().await;

    for relay in relays.values_mut() {
        relay.stop(app).await;
    }
    Ok(())
}
