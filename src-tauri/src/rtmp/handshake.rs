use super::session;

use crate::config::{self};
use rml_rtmp::handshake::{Handshake, HandshakeProcessResult, PeerType};
use std::sync::{atomic::Ordering, Arc};
use tauri::{AppHandle, Manager};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};

pub async fn init_rtmp_server(app: AppHandle, port: u16) {
    let listener = TcpListener::bind(format!("0.0.0.0:{}", port))
        .await
        .expect("Failed to bind");
    println!("🟢 RTMP server listening on rtmp://localhost:1580");
    let app_state = app.state::<Arc<crate::config::AppState>>();
    app_state.rtmp_ready.store(true, Ordering::SeqCst);
    loop {
        let (socket, addr) = listener.accept().await.expect("Failed to accept");
        println!("🔗 Connection from {}", addr);
        if app_state.rtmp_active.swap(true, Ordering::SeqCst) {
            eprintln!("⚠️ Rejecting RTMP connection from {addr}: stream already in use");
            drop(socket); // Close the connection
            continue;
        }
        println!("🔗 Accepted RTMP connection from {addr}");
        let app_clone: AppHandle = app.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_connection(app_clone.clone(), socket).await {
                eprintln!("❌ Error: {}", e);
            }
            let state = app_clone.state::<Arc<config::AppState>>();
            state.rtmp_active.store(false, Ordering::SeqCst);
            println!("📴 RTMP connection ended");
        });
    }
}

async fn handle_connection(
    app: AppHandle,
    mut socket: TcpStream,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("📡 Handling RTMP connection...");

    let mut handshake = Handshake::new(PeerType::Server);
    let mut buffer = [0u8; 4096];
    let mut received_data = Vec::new();

    loop {
        let n = socket.read(&mut buffer).await?;
        if n == 0 {
            return Err("🔌 Connection closed during handshake".into());
        }
        received_data.extend_from_slice(&buffer[..n]);

        match handshake.process_bytes(&received_data) {
            Ok(HandshakeProcessResult::InProgress { response_bytes }) => {
                socket.write_all(&response_bytes).await?;
                received_data.clear(); // Reset buffer until next chunk
            }

            Ok(HandshakeProcessResult::Completed {
                response_bytes,
                remaining_bytes,
            }) => {
                socket.write_all(&response_bytes).await?;
                println!("✅ RTMP handshake complete 🤝");
                return session::handle_session(&app, socket, remaining_bytes.to_vec()).await;
            }

            Err(e) => {
                return Err(format!("❌ Handshake error: {:?}", e).into());
            }
        };
    }
}
