use tokio::io::AsyncReadExt;
use tokio::net::TcpStream;
use uuid::Uuid;

use crate::state::AppState;

type Result<T> = anyhow::Result<T>;

pub async fn process_rtmp_stream(
    socket: &mut TcpStream,
    appsrc: &gstreamer_app::AppSrc,
    initial_data: Vec<u8>,
    peer_addr: &str,
    stream_id: Uuid,
    state: AppState,
) -> Result<()> {
    let mut total_bytes = 0;

    // Push any remaining bytes from the handshake first
    if !initial_data.is_empty() {
        let initial_len = initial_data.len();
        println!(
            "📦 [{}] Pushing {} bytes of initial RTMP data",
            peer_addr, initial_len
        );

        let buffer = gstreamer::Buffer::from_slice(initial_data);
        if appsrc.push_buffer(buffer).is_err() {
            return Err(anyhow::anyhow!("Failed to push initial data to pipeline").into());
        }
        total_bytes += initial_len;
        state.update_stream_bitrate(stream_id, initial_len as u64).await;
    }

    // Main streaming loop
    let mut read_buffer = [0u8; 8192];
    let mut packets_received = 0;

    loop {
        let bytes_read = socket.read(&mut read_buffer).await?;

        if bytes_read == 0 {
            println!(
                "📊 [{}] Stream ended. Total: {} bytes, {} packets",
                peer_addr, total_bytes, packets_received
            );
            break;
        }

        // Push data to GStreamer pipeline
        let buffer = gstreamer::Buffer::from_slice(read_buffer[..bytes_read].to_vec());
        if appsrc.push_buffer(buffer).is_err() {
            println!("⚠️ [{}] Pipeline stopped accepting data", peer_addr);
            break;
        }

        total_bytes += bytes_read;
        packets_received += 1;

        // Update bitrate stats
        state.update_stream_bitrate(stream_id, bytes_read as u64).await;

        // Log progress every 100 packets
        if packets_received % 100 == 0 {
            println!(
                "📈 [{}] Received {} packets ({} bytes)",
                peer_addr, packets_received, total_bytes
            );
        }
    }

    Ok(())
}
