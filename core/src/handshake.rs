use rml_rtmp::handshake::{Handshake, HandshakeProcessResult, PeerType};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
// use tracing::{debug, instrument};

type Result<T> = anyhow::Result<T>;

pub async fn perform_rtmp_handshake(socket: &mut TcpStream, peer_addr: &str) -> Result<Vec<u8>> {
    println!("🤝 [{}] Starting RTMP handshake...", peer_addr);

    let mut handshake = Handshake::new(PeerType::Server);
    let mut buffer = [0u8; 4096];
    let mut handshake_step = 1;

    loop {
        // Read data from the client
        let bytes_read = socket.read(&mut buffer).await?;

        if bytes_read == 0 {
            return Err(anyhow::anyhow!("Connection closed during handshake").into());
        }

        println!(
            "   [{}] Step {}: Received {} bytes",
            peer_addr, handshake_step, bytes_read
        );

        // Process the handshake bytes
        match handshake.process_bytes(&buffer[..bytes_read]) {
            Ok(HandshakeProcessResult::InProgress { response_bytes }) => {
                // Handshake in progress - send response
                if !response_bytes.is_empty() {
                    socket.write_all(&response_bytes).await?;
                    socket.flush().await?;
                    println!(
                        "   [{}] Step {}: Sent {} bytes response",
                        peer_addr,
                        handshake_step,
                        response_bytes.len()
                    );
                }
                handshake_step += 1;
            }
            Ok(HandshakeProcessResult::Completed {
                response_bytes,
                remaining_bytes,
            }) => {
                // Handshake complete - send final response
                if !response_bytes.is_empty() {
                    socket.write_all(&response_bytes).await?;
                    socket.flush().await?;
                    println!(
                        "   [{}] Final: Sent {} bytes response",
                        peer_addr,
                        response_bytes.len()
                    );
                }

                println!(
                    "✅ [{}] RTMP handshake completed successfully! ({} bytes remaining)",
                    peer_addr,
                    remaining_bytes.len()
                );

                return Ok(remaining_bytes);
            }
            Err(e) => {
                return Err(anyhow::anyhow!("Handshake failed: {:?}", e).into());
            }
        }
    }
}
