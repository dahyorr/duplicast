use crate::rtmp::handshake::handle_relay_handshake;
use crate::rtmp::relay_pump::{start_pump_task, start_reader_task};
use crate::{config, db, events::AppEvents, models};
use bytes::Bytes;
use rml_rtmp::sessions::{
    ClientSession, ClientSessionConfig, ClientSessionEvent, ClientSessionResult, PublishRequestType,
};
use std::net::ToSocketAddrs;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::{split, AsyncReadExt, AsyncWriteExt};
use tokio::io::{ReadHalf, WriteHalf};
use tokio::net::TcpStream;
use tokio::sync::{broadcast, Mutex};
use url::Url;

#[derive(Debug)]
struct RelayCredentials {
    url: String,
    stream_key: String,
}

pub struct RelayHandle {
    pub id: i64,
    pub active: Arc<AtomicBool>,
    credentials: RelayCredentials,
    pub rx: broadcast::Receiver<Bytes>,
    pub rx_task: Option<tauri::async_runtime::JoinHandle<()>>,
    pub reader_task: Option<tauri::async_runtime::JoinHandle<()>>,
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
            reader_task: None,
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
        println!(
            "🔵 Relay {} starting connection to {}",
            self.id, self.credentials.url
        );
        let start_time = std::time::Instant::now();

        let url = Url::parse(&self.credentials.url)?;
        let host = url.host_str().ok_or("missing host")?;
        let port = url.port().unwrap_or(1935);

        // TCP dial
        println!("🔌 Relay {} resolving {}:{}", self.id, host, port);
        let addr = (host, port)
            .to_socket_addrs()?
            .next()
            .ok_or("cannot resolve")?;

        println!("🔌 Relay {} connecting to {}...", self.id, addr);
        let socket = TcpStream::connect(addr).await?;
        socket.set_nodelay(true)?;
        println!(
            "✅ Relay {} TCP connected in {:?}",
            self.id,
            start_time.elapsed()
        );

        // handshake & connect
        println!("🤝 Relay {} performing RTMP handshake...", self.id);
        let (socket, remaining) = handle_relay_handshake(socket).await.map_err(
            |e| -> Box<dyn std::error::Error + Send + Sync> {
                eprintln!("❌ Relay {} handshake failed: {}", self.id, e);
                format!("Handshake failed: {}", e).into()
            },
        )?;
        println!(
            "✅ Relay {} handshake complete ({} bytes remaining)",
            self.id,
            remaining.len()
        );

        println!("⚙️ Relay {} setting up RTMP client session...", self.id);
        let (socket, session) = self.setup_rtmp_client_session(socket, remaining).await?;
        println!("✅ Relay {} RTMP session established", self.id);
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
        let mut loop_iterations = 0;

        println!("⏳ Relay {} waiting for publish acceptance...", self.id);
        // Read and process messages until publish is accepted
        while !publish_accepted {
            loop_iterations += 1;
            let n = match reader.lock().await.read(&mut buf).await {
                Ok(0) => {
                    eprintln!("❌ Relay {} server closed connection before publish accepted (after {} iterations)", self.id, loop_iterations);
                    return Err("Server closed connection before publish accepted".into());
                }
                Ok(n) => {
                    println!(
                        "📥 Relay {} received {} bytes (iteration {})",
                        self.id, n, loop_iterations
                    );
                    n
                }
                Err(e) => {
                    eprintln!(
                        "❌ Relay {} read error after {} iterations: {} (kind: {:?})",
                        self.id,
                        loop_iterations,
                        e,
                        e.kind()
                    );
                    return Err(format!("Read error: {}", e).into());
                }
            };

            let responses = session.lock().await.handle_input(&buf[..n])?;
            for res in responses {
                match res {
                    ClientSessionResult::RaisedEvent(event) => {
                        if matches!(event, ClientSessionEvent::PublishRequestAccepted) {
                            publish_accepted = true;
                        }
                        
                        if let Err(e) = self.handle_relay_session_event(app, event).await {
                            eprintln!("⚠️ Relay {} session event error: {}", self.id, e);
                            let reason = e.to_string();
                            self.handle_relay_failed(app, &reason);
                        }
                    }
                    ClientSessionResult::OutboundResponse(pkt) => {
                        println!("📤 Relay {} writing outbound response ({} bytes)", self.id, pkt.bytes.len());
                        writer.lock().await.write_all(&pkt.bytes).await?;
                    }
                    ClientSessionResult::UnhandleableMessageReceived(payload) => {
                        eprintln!("⚠️ Relay {} unhandled RTMP message: {:?}", self.id, payload);
                    }
                }
            }
        }
        Ok(())
    }

    pub async fn stop(&mut self, app: &AppHandle) {
        if self.active.load(Ordering::SeqCst) {
            println!("🛑 Relay {} stopping...", self.id);

            if let Some(task) = self.rx_task.take() {
                println!("   └─ Aborting pump task");
                task.abort();
            }

            if let Some(task) = self.reader_task.take() {
                println!("   └─ Aborting reader task");
                task.abort();
            }

            self.writer = None;
            self.reader = None;
            self.session = None;
            println!("   └─ Cleared connection resources");

            if let Err(e) = app.emit(AppEvents::RelayEnded.as_str(), self.id) {
                eprintln!("   └─ Failed to emit RelayEnded event: {}", e);
            }

            self.active.store(false, Ordering::SeqCst);
            println!("🔴 Relay {} stopped gracefully", self.id);
        } else {
            println!("⚠️ Relay {} is already inactive, skipping stop", self.id);
        }
    }

    pub fn handle_relay_failed(&mut self, app: &AppHandle, reason: &str) {
        eprintln!("🔴 Relay {} FAILED: {}", self.id, reason);
        eprintln!(
            "   └─ State: pump_task={}, reader_task={}, writer={}, reader={}, session={}",
            self.rx_task.is_some(),
            self.reader_task.is_some(),
            self.writer.is_some(),
            self.reader.is_some(),
            self.session.is_some()
        );

        if let Some(task) = self.rx_task.take() {
            println!("   └─ Aborting pump task...");
            task.abort();
        }
        if let Some(task) = self.reader_task.take() {
            println!("   └─ Aborting reader task...");
            task.abort();
        }
        self.writer = None;
        self.reader = None;
        self.session = None;

        if let Err(e) = app.emit(AppEvents::RelayFailed.as_str(), self.id) {
            eprintln!("   └─ Failed to emit RelayFailed event: {}", e);
        }
        self.active.store(false, Ordering::SeqCst);
        eprintln!("   └─ Relay {} cleanup complete", self.id);
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
                println!("✅ Relay {} connection request accepted", self.id);
                let r = session.lock().await.request_publishing(
                    self.credentials.stream_key.clone(),
                    PublishRequestType::Live,
                )?;
                if let ClientSessionResult::OutboundResponse(pkt) = r {
                    writer.lock().await.write_all(&pkt.bytes).await?;
                }
            }
            ClientSessionEvent::ConnectionRequestRejected { description } => {
                eprintln!("❌ Relay {} connection rejected: {}", self.id, description);
            }
            ClientSessionEvent::PublishRequestAccepted => {
                println!("✅ Relay {} publish request accepted", self.id);
                self.start_pump(app).await?;
            }
            ev => {
                println!("ℹ️ Relay {} event: {:?}", self.id, ev);
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
        let reader = self.reader.clone().unwrap();
        let state = app.state::<Arc<config::AppState>>();
        let metadata = state.source_metadata.lock().await;

        println!(
            "📋 Relay {} starting pump with metadata: {:?}",
            self.id,
            metadata.is_some()
        );

        let (ping_pkt, _) = session.lock().await.send_ping_request().unwrap();

        writer.lock().await.write_all(&ping_pkt.bytes).await?;
        println!("🏓 Relay {} ping sent", self.id);

        if let Some(metadata) = metadata.as_ref() {
            println!(
                "📊 Relay {} sending metadata: width={:?}, height={:?}, fps={:?}, bitrate={:?}",
                self.id,
                metadata.video_width,
                metadata.video_height,
                metadata.video_frame_rate,
                metadata.video_bitrate_kbps
            );
            let r = session.lock().await.publish_metadata(metadata)?;
            if let ClientSessionResult::OutboundResponse(pkt) = r {
                writer.lock().await.write_all(&pkt.bytes).await?;
                println!(
                    "✅ Relay {} metadata sent ({} bytes)",
                    self.id,
                    pkt.bytes.len()
                );
            }
        } else {
            eprintln!(
                "⚠️ Relay {} no metadata available - stream may not work properly!",
                self.id
            );
        }

        let rx = self.rx.resubscribe();
        let relay_id = self.id;
        let active = self.active.clone();
        let app_handle = app.clone();

        println!(
            "🎯 Relay {} subscribed to encoder broadcast channel",
            relay_id
        );

        // Start reader task to handle incoming server messages
        let reader_task = start_reader_task(
            relay_id,
            reader,
            session.clone(),
            writer.clone(),
            active.clone(),
            app_handle.clone(),
        );
        self.reader_task = Some(reader_task);

        // Start pump task to send data from encoder to relay server
        let pump_task = start_pump_task(relay_id, rx, session, writer, active, app_handle);
        self.rx_task = Some(pump_task);

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
