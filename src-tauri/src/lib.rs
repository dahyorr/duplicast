use std::{fs::File, io::Write};

use log::{error, info};
use tauri::async_runtime;
use tokio::net::TcpListener;

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![greet])
        .setup(|app| {
            async_runtime::spawn(init_rtmp());
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

async fn init_rtmp() {
    const PORT: &str = "1557";
    let listener = TcpListener::bind(format!("127.0.0.1:{}", PORT)).await;

    match listener {
        Ok(listener) => {
            info!("Listening on: {}", listener.local_addr().unwrap());
            if let Err(e) = handle_incoming_connections(listener).await {
                error!("Error handling connections: {:?}", e);
            }
        }
        Err(e) => {
            error!("Failed to bind to port {}: {:?}", PORT, e);
            // try another port
        }
    }
}

async fn handle_incoming_connections(
    listener: TcpListener,
) -> Result<(), Box<dyn std::error::Error>> {
    loop {
        let (stream, connection_info) = listener.accept().await?;
        info!("New connection from: {}", connection_info);

        tokio::spawn(async move {
            if let Err(e) = handle_connection(&stream).await {
                error!(
                    "Error handling connection from {}: {:?}",
                    connection_info, e
                );
            }
        });
        ()
    }
}

async fn handle_connection(
    stream: &tokio::net::TcpStream,
) -> Result<(), Box<dyn std::error::Error>> {
    // let mut session = ServerSession::new();
    // let mut deserializer = ChunkDeserializer::new();
    // let mut serializer = ChunkSerializer::new();

    // loop {
    //     let mut buffer = [0u8; 1024];
    //     let bytes_read = socket.read(&mut buffer).await?;

    //     if bytes_read == 0 {
    //         break;
    //     }

    //     // Deserialize RTMP messages
    //     let messages = deserializer.get_messages(&buffer[..bytes_read])?;
    //     for message in messages {
    //         // Process each message (e.g., publish, play, etc.)
    //         if let Some(payload) = message.message_payload {
    //             match payload {
    //                 MessagePayload::VideoData(data) => {
    //                     // Handle video data
    //                 }
    //                 MessagePayload::AudioData(data) => {
    //                     // Handle audio data
    //                 }
    //                 _ => {}
    //             }
    //         }
    //     }

    let mut buffer = [0u8; 1024];

    loop {
        let bytes_read = stream.try_read(&mut buffer)?;
        if bytes_read == 0 {
            info!("Client disconnected");
            break;
        }
        // For demonstration: log the bytes read
        info!("Received {} bytes", bytes_read);
        // write bytes to file
        let mut file = File::create("output.flv")?;
        file.write_all(&buffer[..bytes_read])?;

        // Here you would process the RTMP data
        // This is a placeholder; actual RTMP parsing would go here
    }

    Ok(())
}
