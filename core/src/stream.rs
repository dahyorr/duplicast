use tokio::io::AsyncReadExt;
use tokio::net::TcpStream;
use tracing::{debug, error, info, instrument};
use uuid::Uuid;

use crate::state::AppState;

type Result<T> = anyhow::Result<T>;

#[instrument(skip(socket, appsrc, initial_data, state), fields(stream_id = %stream_id, client_addr = %peer_addr))]
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
        debug!(bytes = initial_len, "Pushing initial RTMP data");

        let buffer = gstreamer::Buffer::from_slice(initial_data);
        if appsrc.push_buffer(buffer).is_err() {
            return Err(anyhow::anyhow!("Failed to push initial data to pipeline").into());
        }
        total_bytes += initial_len;
        state.update_stream_bitrate(stream_id, initial_len as u64).await;
    }
    
    info!("Starting stream data processing");

    // Main streaming loop with larger buffer for better throughput
    let mut read_buffer = [0u8; 65536]; // 64KB buffer
    let mut packets_received = 0;

    loop {
        let bytes_read = socket.read(&mut read_buffer).await?;

        if bytes_read == 0 {
            tracing::info!(
                peer_addr, total_bytes, packets_received,
                "Stream ended"
            );
            break;
        }

        // Push data to GStreamer pipeline
        let buffer = gstreamer::Buffer::from_slice(read_buffer[..bytes_read].to_vec());
        if let Err(e) = appsrc.push_buffer(buffer) {
            error!(error = ?e, "Pipeline stopped accepting data");
            break;
        }

        total_bytes += bytes_read;
        packets_received += 1;

        // Update bitrate stats
        state.update_stream_bitrate(stream_id, bytes_read as u64).await;

        // Log progress every 1000 packets
        if packets_received % 1000 == 0 {
            debug!(
                packets_received,
                mb = total_bytes / 1024 / 1024,
                "Stream progress"
            );
        }
    }

    info!(
        packets_received,
        mb = total_bytes / 1024 / 1024,
        "Stream processing completed"
    );

    Ok(())
}
