use crate::rtmp::handshake::handle_relay_handshake;
use crate::rtmp::utils::{peek_flv_tag, FlvTagType};
use crate::{config, db, events::AppEvents, models};
use bytes::Bytes;
use rml_rtmp::sessions::{
    ClientSession, ClientSessionConfig, ClientSessionEvent, ClientSessionResult, PublishRequestType,
};
use rml_rtmp::time::RtmpTimestamp;
use std::net::ToSocketAddrs;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::AsyncReadExt;
use tokio::net::TcpStream;
use tokio::{
    io::AsyncWriteExt,
    sync::{broadcast, Mutex},
};
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
    pub rx: broadcast::Receiver<Vec<u8>>,
    pub rx_task: Option<tauri::async_runtime::JoinHandle<()>>,
    pub socket: Option<Arc<Mutex<TcpStream>>>,
    pub session: Option<Arc<Mutex<ClientSession>>>,
    // pub tx: mpsc::Sender<Arc<Vec<u8>>>,
}

impl RelayHandle {
    pub fn from_relay_target(
        relay: &models::RelayTarget,
        encoder_rx: broadcast::Receiver<Vec<u8>>,
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
            socket: None,
            session: None,
        }
    }

    pub async fn start(&mut self, app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
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
        let (socket, remaining) = handle_relay_handshake(socket).await?;
        let (socket, session) = self.setup_rtmp_client_session(socket, remaining).await?;

        let socket = Arc::new(Mutex::new(socket));
        let session = Arc::new(Mutex::new(session));

        self.socket = Some(socket.clone());
        self.session = Some(session.clone());

        // feed any leftover handshake bytes into the session
        let mut buf = [0u8; 4096];

        loop {
            let n = socket.lock().await.read(&mut buf).await.unwrap();
            if n == 0 {
                return Err("RTMP server closed connection".into());
            }
            let responses = session.lock().await.handle_input(&buf[..n])?;
            for res in responses {
                match res {
                    ClientSessionResult::RaisedEvent(event) => {
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
                        socket.lock().await.write_all(&pkt.bytes).await?;
                    }
                    ClientSessionResult::UnhandleableMessageReceived(payload) => {
                        eprintln!("RTMP Unhandled: {:?}", payload);
                    }
                }
            }
        }
    }

    pub async fn stop(&mut self, app: &AppHandle) {
        if self.active.load(Ordering::SeqCst) {
            // 1. Abort the pump task if it's running
            if let Some(task) = self.rx_task.take() {
                task.abort(); // Stops the background spawn
            }

            // 3. Clear socket/session (optional)
            self.socket = None;
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
        self.socket = None;
        self.session = None;

        let _ = app.emit(AppEvents::RelayFailed.as_str(), self.id);
        self.active.store(false, Ordering::SeqCst);
    }

    async fn setup_rtmp_client_session(
        &mut self,
        mut socket: TcpStream,
        remaining: Vec<u8>,
    ) -> Result<(TcpStream, ClientSession), Box<dyn std::error::Error>> {
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
    ) -> Result<(), Box<dyn std::error::Error>> {
        let session = self.session.clone().unwrap();
        let socket = self.socket.clone().unwrap();
        match event {
            ClientSessionEvent::ConnectionRequestAccepted => {
                let r = session.lock().await.request_publishing(
                    self.credentials.stream_key.clone(),
                    PublishRequestType::Live,
                )?;
                if let ClientSessionResult::OutboundResponse(pkt) = r {
                    socket.lock().await.write_all(&pkt.bytes).await?;
                }
            }
            ClientSessionEvent::ConnectionRequestRejected { description } => {
                println!("Connection Failed: {description}");
            }
            ClientSessionEvent::PublishRequestAccepted => {
                let state = app.state::<Arc<config::AppState>>();
                let metadata = state.source_metadata.lock().await;
                if let Some(metadata) = metadata.as_ref() {
                    let r = session.lock().await.publish_metadata(metadata)?;
                    if let ClientSessionResult::OutboundResponse(pkt) = r {
                        socket.lock().await.write_all(&pkt.bytes).await?;
                    }
                }

                self.start_pump(app).await?;
            }
            ClientSessionEvent::AcknowledgementReceived {
                bytes_received: _bytes_received,
            } => {
                // println!("Acknowledgement Received: {_bytes_received}");
            }
            ClientSessionEvent::PingResponseReceived {
                timestamp: _timestamp,
            } => {
                println!("Ping Response Received");
            }
            ev => {
                println!("Unknown event {:?}", ev);
            }
        }
        Ok(())
    }

    async fn start_pump(&mut self, app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
        let session = self.session.clone().unwrap();
        let socket = self.socket.clone().unwrap();
        let mut rx = self.rx.resubscribe();
        let task = tauri::async_runtime::spawn(async move {
            while let Ok(chunk) = rx.recv().await {
                if let Some((tag_type, _data_size, timestamp)) = peek_flv_tag(&chunk) {
                    let data = Bytes::from(chunk);
                    let timestamp = RtmpTimestamp::new(timestamp);
                    let resp = match tag_type {
                        FlvTagType::Audio => session
                            .lock()
                            .await
                            .publish_audio_data(data, timestamp, false)
                            .unwrap(),
                        FlvTagType::Video => session
                            .lock()
                            .await
                            .publish_video_data(data, timestamp, false)
                            .unwrap(),
                        _ => continue,
                    };
                    if let ClientSessionResult::OutboundResponse(pkt) = resp {
                        let _ = socket.lock().await.write_all(&pkt.bytes).await;
                    }
                }
            }
        });
        self.rx_task = Some(task);

        // 3) notify the UI
        app.emit(AppEvents::RelayActive.as_str(), self.id)?;
        println!("🟢 Relay {} started", self.id);
        self.active.store(true, Ordering::SeqCst);
        Ok(())
    }
}

pub async fn start_relays(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let pool = db::get_db_pool();
    let relays = db::get_active_relay_targets(&pool).await?;
    let state = app.state::<Arc<config::AppState>>();
    let tx = state.encoder_tx.clone();

    for relay in relays {
        let mut relay_handle = RelayHandle::from_relay_target(&relay, tx.subscribe());
        relay_handle.start(app).await?;
        state.relays.lock().await.insert(relay.id, relay_handle);
    }
    Ok(())
}

pub async fn stop_relays(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let state = app.state::<Arc<config::AppState>>();
    let mut relays = state.relays.lock().await;

    for relay in relays.values_mut() {
        relay.stop(app).await;
    }
    Ok(())
}
