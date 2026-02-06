# Duplicast Core

A high-performance RTMP streaming server with built-in management API, relay support, bitrate monitoring, and WebRTC preview capabilities.

## Features

- ✅ **RTMP Ingest Server** - Accepts RTMP streams on port 1935
- ✅ **Management REST API** - HTTP API on port 8080
- ✅ **Stream Monitoring** - Real-time bitrate and statistics tracking
- ✅ **Relay Management** - Add, start, stop, and monitor relay endpoints
- ✅ **WebRTC Preview** - Live preview of streams via WebRTC (planned)
- ✅ **GStreamer Pipeline** - H.264/AAC video/audio processing

## Quick Start

```bash
cargo run
```

This starts:
- RTMP Server: `rtmp://localhost:1935/live`
- Management API: `http://localhost:8080`

## Streaming to the Server

Using OBS Studio:
1. Settings → Stream
2. Service: Custom
3. Server: `rtmp://localhost:1935/live`
4. Stream Key: `any_key_you_want`

Using FFmpeg:
```bash
ffmpeg -re -i input.mp4 -c copy -f flv rtmp://localhost:1935/live/mystream
```

## Management API

### Health Check
```bash
GET /api/health
```

### Statistics
```bash
GET /api/stats
```
Returns:
```json
{
  "active_streams": 1,
  "total_relays": 2,
  "active_relays": 1,
  "total_bitrate": 2500000,
  "total_bitrate_mbps": 2.5,
  "total_bytes": 1048576
}
```

### Streams

**List all streams**
```bash
GET /api/streams
```

**Get specific stream**
```bash
GET /api/streams/:id
```

**Get stream with relays**
```bash
GET /api/streams/:id/info
```

### Relays

**List all relays**
```bash
GET /api/relays
```

**Create a relay**
```bash
POST /api/relays
Content-Type: application/json

{
  "name": "YouTube",
  "target_url": "rtmp://a.rtmp.youtube.com/live2/your-stream-key"
}
```

**Get relay**
```bash
GET /api/relays/:id
```

**Start relay**
```bash
POST /api/relays/:id/start
Content-Type: application/json

{
  "stream_id": "uuid-of-stream"
}
```

**Stop relay**
```bash
POST /api/relays/:id/stop
```

**Delete relay**
```bash
DELETE /api/relays/:id
```

### WebRTC Preview

**Submit WebRTC offer for stream preview**
```bash
POST /api/streams/:id/webrtc/offer
Content-Type: application/json

{
  "sdp": "...",
  "type": "offer"
}
```

## Example Usage

### 1. Start streaming to the server
```bash
ffmpeg -re -i video.mp4 -c copy -f flv rtmp://localhost:1935/live/test
```

### 2. Check active streams
```bash
curl http://localhost:8080/api/streams
```

### 3. Create a relay to YouTube
```bash
curl -X POST http://localhost:8080/api/relays \
  -H "Content-Type: application/json" \
  -d '{
    "name": "YouTube Live",
    "target_url": "rtmp://a.rtmp.youtube.com/live2/YOUR-KEY"
  }'
```

### 4. Start the relay
```bash
# Get the stream_id from step 2 and relay_id from step 3
curl -X POST http://localhost:8080/api/relays/RELAY-ID/start \
  -H "Content-Type: application/json" \
  -d '{"stream_id": "STREAM-ID"}'
```

### 5. Monitor stats
```bash
curl http://localhost:8080/api/stats
```

## Architecture

```
┌─────────────┐
│   Publisher │ (OBS/FFmpeg)
└──────┬──────┘
       │ RTMP
       ▼
┌─────────────────┐
│  RTMP Server    │ :1935
│  - Handshake    │
│  - Session Mgmt │
│  - Monitoring   │
└────────┬────────┘
         │
         ▼
┌─────────────────┐     ┌──────────────┐
│  GStreamer      │────▶│   Relays     │ → YouTube/Twitch/etc
│  - FLV Demux    │     └──────────────┘
│  - H264 Parse   │
│  - AAC Parse    │     ┌──────────────┐
└─────────────────┘────▶│  WebRTC Out  │ → Browser Preview
                        └──────────────┘
         │
         ▼
┌─────────────────┐
│ Management API  │ :8080
│  - Stats        │
│  - Relays       │
│  - Monitoring   │
└─────────────────┘
```

## Module Structure

- `main.rs` - Entry point
- `server.rs` - RTMP server and connection handling
- `handshake.rs` - RTMP handshake protocol
- `session.rs` - RTMP session management
- `stream.rs` - Stream processing with monitoring
- `pipeline.rs` - GStreamer pipeline setup
- `state.rs` - Shared application state
- `types.rs` - Data structures
- `management.rs` - REST API server

## Dependencies

- `tokio` - Async runtime
- `rml_rtmp` - RTMP protocol
- `gstreamer` - Media processing
- `axum` - Web framework
- `serde` - Serialization
- `uuid` - Unique identifiers

## License

MIT
