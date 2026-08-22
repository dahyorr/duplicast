use rml_rtmp::handshake::{Handshake, HandshakeProcessResult, PeerType};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

type Result<T> = anyhow::Result<T>;

pub async fn perform_rtmp_handshake(socket: &mut TcpStream, peer_addr: &str) -> Result<Vec<u8>> {
    tracing::debug!(peer_addr, "Starting RTMP handshake");

    let mut handshake = Handshake::new(PeerType::Server);
    let mut buffer = [0u8; 4096];
    let mut handshake_step = 1;

    loop {
        let bytes_read = socket.read(&mut buffer).await?;

        if bytes_read == 0 {
            return Err(anyhow::anyhow!("Connection closed during handshake").into());
        }

        tracing::debug!(peer_addr, step = handshake_step, bytes_read, "Handshake bytes received");

        match handshake.process_bytes(&buffer[..bytes_read]) {
            Ok(HandshakeProcessResult::InProgress { response_bytes }) => {
                if !response_bytes.is_empty() {
                    socket.write_all(&response_bytes).await?;
                    socket.flush().await?;
                    tracing::debug!(peer_addr, step = handshake_step, sent = response_bytes.len(), "Sent handshake response");
                }
                handshake_step += 1;
            }
            Ok(HandshakeProcessResult::Completed {
                response_bytes,
                remaining_bytes,
            }) => {
                if !response_bytes.is_empty() {
                    socket.write_all(&response_bytes).await?;
                    socket.flush().await?;
                    tracing::debug!(peer_addr, sent = response_bytes.len(), "Sent final handshake response");
                }

                tracing::debug!(peer_addr, remaining = remaining_bytes.len(), "RTMP handshake complete");

                return Ok(remaining_bytes);
            }
            Err(e) => {
                return Err(anyhow::anyhow!("Handshake failed: {:?}", e).into());
            }
        }
    }
}
