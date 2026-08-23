# WebRTC Stream Preview

## Overview

The `StreamPreview` component provides a low-latency WebRTC video preview of a
live RTMP stream, using a real GStreamer `webrtcbin`-based signaling bridge
on the backend (not a mock or placeholder).

## Component Features

- **WebRTC Integration**: Uses `RTCPeerConnection` for real-time video streaming
- **Playback Controls**: Play/pause, mute/unmute, fullscreen support
- **Loading States**: Visual feedback during connection establishment
- **Error Handling**: Graceful error display with retry functionality
- **Responsive Design**: Adapts to different screen sizes with proper aspect ratio
- **Hover Controls**: Shows video controls on mouse hover
- **Clean teardown**: Hangs up the server-side WebRTC session on unmount/pause
  so viewer sessions don't leak GStreamer elements

## Usage

```tsx
import { StreamPreview } from "@/components/StreamPreview"

<StreamPreview
  streamUrl={`rtmp://localhost:1935/${stream.app_name}/${stream.stream_key}`}
  streamId={stream.id}
  autoPlay={false}
/>
```

## Props

| Prop | Type | Default | Description |
|------|------|---------|-------------|
| `streamUrl` | `string \| undefined` | - | Display-only RTMP URL of the stream being previewed |
| `streamId` | `string \| undefined` | - | Stream ID used to address the WebRTC signaling endpoints |
| `className` | `string \| undefined` | - | Additional CSS classes |
| `autoPlay` | `boolean` | `false` | Whether to auto-play on mount |

## How signaling works

1. The browser creates an `RTCPeerConnection`, generates an offer, and waits
   for ICE gathering to complete (no trickle ICE - the offer already contains
   every candidate by the time it's sent).
2. The offer is POSTed to `POST /api/streams/{id}/webrtc/offer`
   (`sendWebRTCOffer` in `client/src/api.ts`).
3. The backend handler (`webrtc_offer` in `core/src/management.rs`) attaches a
   `webrtcbin` + encoder chain to the stream's live GStreamer pipeline
   (`attach_webrtc_to_pipeline` in `core/src/state.rs`), waits for its own ICE
   gathering to finish, and returns an SDP answer plus a `session_id`.
4. The browser applies the answer via `setRemoteDescription` and starts
   receiving the video/audio tracks.
5. On unmount or pause, the browser calls
   `DELETE /api/streams/{id}/webrtc/{session_id}` (`sendWebRTCHangup`), which
   detaches and cleans up that viewer's GStreamer elements
   (`AppState::remove_webrtc_session`). Without this the session's elements
   would stay attached to the pipeline until the whole stream ended.

The STUN server used by both sides is `Config.stun_server` (settable from the
Settings page), not a hardcoded constant.

## Auth

If the server has `DUPLICAST_API_TOKEN` set, mutating endpoints (relay
create/start/stop/delete, config updates, WebRTC hangup) require a
`Authorization: Bearer <token>` header. The frontend sends this automatically
if built with `VITE_API_TOKEN` set to the same value. The `webrtc/offer` and
`webrtc/ice` endpoints themselves are intentionally left open (same trust
level as viewing the stream).

## Known limitations

- ICE is gather-then-send only; there's no trickle ICE support. This is fine
  on typical local/LAN setups but can add latency to connection setup on
  networks with slow STUN round-trips.
- No TURN server support - if both the server and viewer are behind
  symmetric NAT, the preview may fail to connect. Not an issue for the common
  case of previewing from the same network as the server.
- A crashed/killed browser tab (not a normal unmount) won't call the hangup
  endpoint, so that session's elements stay attached until the underlying
  stream ends. A `webrtcbin` connection-state watchdog for this case is a
  possible future improvement but isn't implemented.

## Browser Compatibility

WebRTC is supported in all modern browsers (Chrome/Edge, Firefox, Safari,
Opera). No HTTPS requirement here since the preview only receives media
(getUserMedia's HTTPS requirement doesn't apply).
