use rml_rtmp::sessions::{
    ServerSession, ServerSessionConfig, ServerSessionEvent, ServerSessionResult,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use uuid::Uuid;

use crate::flv;
use crate::pipeline::setup_gstreamer_pipeline;
use crate::state::AppState;
use crate::types::{LogEntry, LogLevel};

type Result<T> = anyhow::Result<T>;

/// Once a publish request is accepted, this holds the appsrc media is pushed
/// into and the stream_id it's registered under.
struct MediaSink {
    appsrc: gstreamer_app::AppSrc,
    stream_id: Uuid,
}

/// Drives one RTMP connection for its entire life: connect/publish negotiation
/// via `rml_rtmp::ServerSession`, then (once a publish is accepted) wraps each
/// audio/video message as an FLV tag and pushes it into the stream's GStreamer
/// pipeline. Returns once the client disconnects or finishes publishing.
pub async fn run_rtmp_session(
    socket: &mut TcpStream,
    initial_bytes: Vec<u8>,
    peer_addr: &str,
    state: AppState,
) -> Result<()> {
    tracing::debug!(peer_addr, "Handling RTMP session messages");

    let config = ServerSessionConfig::new();
    let (mut session, initial_results) = ServerSession::new(config)?;
    let mut buffer = initial_bytes;
    let mut read_buffer = vec![0u8; 65536]; // 64KB buffer
    let mut stream_key = String::new();
    #[allow(unused_assignments)]
    let mut app_name = String::new();
    let mut media: Option<MediaSink> = None;

    for result in initial_results {
        if let ServerSessionResult::OutboundResponse(packet) = result {
            socket.write_all(&packet.bytes).await?;
            socket.flush().await?;
        }
    }

    'session: loop {
        let results = session.handle_input(&buffer)?;
        buffer.clear();

        for result in results {
            match result {
                ServerSessionResult::OutboundResponse(packet) => {
                    socket.write_all(&packet.bytes).await?;
                    socket.flush().await?;
                }
                ServerSessionResult::RaisedEvent(event) => match event {
                    ServerSessionEvent::ConnectionRequested {
                        request_id,
                        app_name: app,
                    } => {
                        tracing::debug!(peer_addr, app = %app, "Connection requested");
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

                        app_name = app.clone();
                        stream_key = key.clone();

                        let responses = session.accept_request(request_id)?;
                        for response in responses {
                            if let ServerSessionResult::OutboundResponse(packet) = response {
                                socket.write_all(&packet.bytes).await?;
                                socket.flush().await?;
                            }
                        }

                        tracing::info!(peer_addr, key = %key, "Stream key accepted, starting media transmission");

                        let stream_id = state
                            .register_stream(stream_key.clone(), app_name.clone(), peer_addr.to_string())
                            .await;
                        state
                            .add_log(LogEntry {
                                id: Uuid::new_v4().to_string(),
                                timestamp: chrono::Utc::now().to_rfc3339(),
                                level: LogLevel::Info,
                                message: format!("Stream '{}' connected from {}", stream_key, peer_addr),
                                source: "rtmp".to_string(),
                            })
                            .await;

                        let pipeline_setup = match setup_gstreamer_pipeline(peer_addr) {
                            Ok(setup) => setup,
                            Err(e) => {
                                state.unregister_stream(stream_id).await;
                                return Err(e);
                            }
                        };
                        let appsrc = pipeline_setup.appsrc.clone();
                        state.register_pipeline(stream_id, pipeline_setup).await;

                        if appsrc
                            .push_buffer(gstreamer::Buffer::from_slice(flv::flv_header()))
                            .is_err()
                        {
                            tracing::warn!(peer_addr, "Pipeline rejected FLV header");
                        }

                        media = Some(MediaSink { appsrc, stream_id });
                    }
                    ServerSessionEvent::PublishStreamFinished {
                        app_name: app,
                        stream_key: key,
                    } => {
                        tracing::info!(peer_addr, app = %app, key = %key, "Publish stream finished");
                        break 'session;
                    }
                    ServerSessionEvent::StreamMetadataChanged {
                        app_name,
                        stream_key,
                        metadata,
                    } => {
                        tracing::debug!(app_name = %app_name, stream_key = %stream_key, ?metadata, "Stream metadata changed");
                    }
                    ServerSessionEvent::VideoDataReceived {
                        data, timestamp, ..
                    } => {
                        if let Some(sink) = &media {
                            let tag = flv::wrap_video_tag(timestamp.value, &data);
                            let tag_len = tag.len() as u64;
                            let is_seq_header = data.len() >= 2 && data[1] == 0;
                            state
                                .flv_publish_video(sink.stream_id, bytes::Bytes::from(tag.clone()), is_seq_header)
                                .await;
                            if sink
                                .appsrc
                                .push_buffer(gstreamer::Buffer::from_slice(tag))
                                .is_err()
                            {
                                tracing::warn!(peer_addr, "Pipeline stopped accepting video data");
                            }
                            state.update_stream_bitrate(sink.stream_id, tag_len).await;
                        }
                    }
                    ServerSessionEvent::AudioDataReceived {
                        data, timestamp, ..
                    } => {
                        if let Some(sink) = &media {
                            let tag = flv::wrap_audio_tag(timestamp.value, &data);
                            let tag_len = tag.len() as u64;
                            let is_seq_header = data.len() >= 2 && data[1] == 0;
                            state
                                .flv_publish_audio(sink.stream_id, bytes::Bytes::from(tag.clone()), is_seq_header)
                                .await;
                            if sink
                                .appsrc
                                .push_buffer(gstreamer::Buffer::from_slice(tag))
                                .is_err()
                            {
                                tracing::warn!(peer_addr, "Pipeline stopped accepting audio data");
                            }
                            state.update_stream_bitrate(sink.stream_id, tag_len).await;
                        }
                    }
                    _ => {
                        tracing::debug!(?event, "Other RTMP event");
                    }
                },
                ServerSessionResult::UnhandleableMessageReceived(_payload) => {
                    // Ignore unhandleable messages
                }
            }
        }

        // Read more data from socket
        let bytes_read = socket.read(&mut read_buffer).await?;
        if bytes_read == 0 {
            tracing::info!(peer_addr, "Connection closed");
            break 'session;
        }

        buffer.extend_from_slice(&read_buffer[..bytes_read]);
    }

    if let Some(sink) = media {
        state.unregister_stream(sink.stream_id).await;
        tracing::info!(stream_id = %sink.stream_id, "Stream unregistered");
        state
            .add_log(LogEntry {
                id: Uuid::new_v4().to_string(),
                timestamp: chrono::Utc::now().to_rfc3339(),
                level: LogLevel::Info,
                message: format!("Stream '{}' disconnected", stream_key),
                source: "rtmp".to_string(),
            })
            .await;
    }

    Ok(())
}
