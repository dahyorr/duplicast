# WebRTC Stream Preview

## Overview

The StreamPreview component provides a WebRTC-based video player for previewing live RTMP streams in the browser. This implementation uses the WebRTC API to establish a peer-to-peer connection for low-latency stream viewing.

## Component Features

- **WebRTC Integration**: Uses RTCPeerConnection for real-time video streaming
- **Playback Controls**: Play/pause, mute/unmute, fullscreen support
- **Loading States**: Visual feedback during connection establishment
- **Error Handling**: Graceful error display with retry functionality
- **Responsive Design**: Adapts to different screen sizes with proper aspect ratio
- **Hover Controls**: Shows video controls on mouse hover

## Usage

```tsx
import { StreamPreview } from "@/components/StreamPreview"

// Basic usage
<StreamPreview streamUrl="rtmp://localhost:1935/live/stream" />

// With options
<StreamPreview 
  streamUrl="rtmp://localhost:1935/live/stream"
  className="w-full"
  autoPlay={true}
/>
```

## Props

| Prop | Type | Default | Description |
|------|------|---------|-------------|
| `streamUrl` | `string \| undefined` | - | The RTMP URL of the stream to preview |
| `className` | `string \| undefined` | - | Additional CSS classes |
| `autoPlay` | `boolean` | `false` | Whether to auto-play on mount |

## Implementation Notes

### Backend Requirements

To fully implement WebRTC streaming, you need to add a signaling server to your backend:

1. **WebRTC Signaling Endpoint**: Create an API endpoint that:
   - Accepts SDP offers from clients
   - Converts RTMP stream to WebRTC
   - Returns SDP answer with ICE candidates

2. **RTMP to WebRTC Bridge**: Use tools like:
   - [Janus Gateway](https://janus.conf.meetecho.com/)
   - [Kurento Media Server](https://www.kurento.org/)
   - [Mediasoup](https://mediasoup.org/)
   - GStreamer with webrtcbin

### Example Backend Flow

```rust
// Pseudo-code for signaling endpoint
async fn webrtc_offer(offer: SdpOffer) -> Result<SdpAnswer> {
    // 1. Create WebRTC peer connection
    let peer = create_peer_connection()?;
    
    // 2. Set remote description from client offer
    peer.set_remote_description(offer)?;
    
    // 3. Add stream track from RTMP pipeline
    let stream = get_rtmp_stream()?;
    peer.add_track(stream)?;
    
    // 4. Create answer
    let answer = peer.create_answer()?;
    peer.set_local_description(&answer)?;
    
    Ok(answer)
}
```

### Current Limitations

The current implementation is a **placeholder** that:
- Creates the WebRTC peer connection
- Sends offers to console (not to server)
- Displays UI and controls

**To make it functional**, you must:
1. Implement signaling server endpoint
2. Send offers to backend via HTTP/WebSocket
3. Receive and apply SDP answers
4. Handle ICE candidate exchange

### Integration Points

The component is integrated in:
- **Dashboard Page**: Shows preview when stream is active
- **Stream Page**: Displays in the monitoring view

## Browser Compatibility

WebRTC is supported in all modern browsers:
- Chrome/Edge 23+
- Firefox 22+
- Safari 11+
- Opera 18+

Note: Some browsers require HTTPS for WebRTC getUserMedia (though not needed for receiving streams).

## Future Enhancements

- [ ] WebSocket-based signaling for real-time updates
- [ ] Support for multiple stream qualities
- [ ] Picture-in-picture mode
- [ ] Stream statistics overlay (bitrate, FPS, resolution)
- [ ] Recording functionality
- [ ] Screenshot capture
- [ ] HLS fallback for unsupported browsers
