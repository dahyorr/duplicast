use rml_rtmp::sessions::{
    ServerSession, ServerSessionConfig, ServerSessionEvent, ServerSessionResult,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

type Result<T> = anyhow::Result<T>;

pub async fn handle_rtmp_session(
    socket: &mut TcpStream,
    initial_bytes: Vec<u8>,
    peer_addr: &str,
) -> Result<(Vec<u8>, String, String)> {
    tracing::debug!(peer_addr, "Handling RTMP session messages");

    let config = ServerSessionConfig::new();
    let (mut session, initial_results) = ServerSession::new(config)?;
    let mut buffer = initial_bytes;
    let mut read_buffer = vec![0u8; 65536]; // 64KB buffer
    let mut stream_key = String::new();
    let mut app_name = String::new();

    // Process initial results from session creation
    for result in initial_results {
        if let ServerSessionResult::OutboundResponse(packet) = result {
            socket.write_all(&packet.bytes).await?;
            socket.flush().await?;
        }
    }

    loop {
        // Process buffered data
        let results = session.handle_input(&buffer)?;
        buffer.clear();

        for result in results {
            match result {
                ServerSessionResult::OutboundResponse(packet) => {
                    // Send response to client
                    socket.write_all(&packet.bytes).await?;
                    socket.flush().await?;
                }
                ServerSessionResult::RaisedEvent(event) => {
                    match event {
                        ServerSessionEvent::ConnectionRequested {
                            request_id,
                            app_name: app,
                        } => {
                            tracing::debug!(peer_addr, app = %app, "Connection requested");
                            app_name = app;
                            // Accept any connection request
                            let responses = session.accept_request(request_id)?;
                            for response in responses {
                                if let ServerSessionResult::OutboundResponse(packet) = response {
                                    socket.write_all(&packet.bytes).await?;
                                    socket.flush().await?;
                                }
                            }
                        }
                        ServerSessionEvent::ReleaseStreamRequested {
                            request_id,
                            app_name: app,
                            stream_key: key,
                        } => {
                            tracing::debug!(peer_addr, app = %app, key = %key, "Release stream");
                            stream_key = key;
                            let responses = session.accept_request(request_id)?;
                            for response in responses {
                                if let ServerSessionResult::OutboundResponse(packet) = response {
                                    socket.write_all(&packet.bytes).await?;
                                    socket.flush().await?;
                                }
                            }
                        }
                        ServerSessionEvent::PublishStreamRequested {
                            request_id,
                            app_name: app,
                            stream_key: key,
                            mode,
                        } => {
                            tracing::info!(
                                peer_addr, app = %app, key = %key, ?mode,
                                "Publish stream requested"
                            );

                            app_name = app;
                            stream_key = key.clone();

                            // Accept the publish request
                            let responses = session.accept_request(request_id)?;
                            for response in responses {
                                if let ServerSessionResult::OutboundResponse(packet) = response {
                                    socket.write_all(&packet.bytes).await?;
                                    socket.flush().await?;
                                }
                            }

                            tracing::info!(peer_addr, key = %key, "Stream key accepted, starting media transmission");

                            // Return empty buffer with stream info
                            return Ok((Vec::new(), stream_key, app_name));
                        }
                        ServerSessionEvent::StreamMetadataChanged {
                            app_name,
                            stream_key,
                            metadata,
                        } => {
                            tracing::debug!(app_name = %app_name, stream_key = %stream_key, ?metadata, "Stream metadata changed");
                        }
                        ServerSessionEvent::VideoDataReceived {
                            app_name: _,
                            stream_key: _,
                            data,
                            timestamp: _,
                        } => {
                            tracing::debug!("First video packet received");
                            return Ok((data.to_vec(), stream_key, app_name));
                        }
                        ServerSessionEvent::AudioDataReceived {
                            app_name: _,
                            stream_key: _,
                            data,
                            timestamp: _,
                        } => {
                            tracing::debug!("First audio packet received");
                            return Ok((data.to_vec(), stream_key, app_name));
                        }
                        _ => {
                            tracing::debug!(?event, "Other RTMP event");
                        }
                    }
                }
                ServerSessionResult::UnhandleableMessageReceived(_payload) => {
                    // Ignore unhandleable messages
                }
            }
        }

        // Read more data from socket
        let bytes_read = socket.read(&mut read_buffer).await?;
        if bytes_read == 0 {
            return Err(anyhow::anyhow!("Connection closed during RTMP session setup").into());
        }

        buffer.extend_from_slice(&read_buffer[..bytes_read]);
    }
}
