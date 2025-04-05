# Duplicast

Duplicast is a tool for relaying live streams to multiple destinations. It receives an RTMP stream (e.g. from OBS), encodes to h.264, outputs it to HLS for local preview, and forwards the stream to configured relay targets like YouTube, Twitch, and others.

## Features

- RTMP ingest server
- HLS preview output
- Multi-destination relays using FFmpeg
- Add/remove relays on the fly

## Getting Started

1. Install dependencies:

```bash
cargo install tauri-cli
npm install
```

	2.	Run in dev mode:

```bash
npm run tauri dev
```

	3.	In OBS or similar:
	•	Stream to: rtmp://localhost:1580/live
	•	Use any stream key

Preview HLS at: http://localhost:8787/hls/playlist.m3u8

TODO
	•	Frontend stats for relays (bitrate, status)
	•	Custom encoder config UI
	•	Graceful relay reconnect
	•	Settings persistence

