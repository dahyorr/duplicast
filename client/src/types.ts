export interface Stream {
  id: string;
  stream_key: string;
  app_name: string;
  publisher_addr: string;
  started_at: string;
  bitrate: BitrateStats;
  status: StreamStatus;
}

export interface BitrateStats {
  video_bitrate: number;
  audio_bitrate: number;
  total_bitrate: number;
  total_bytes: number;
  packets_received: number;
  last_updated: string | null;
}

export type StreamStatus =
  | { status: 'active' }
  | { status: 'inactive' }
  | { status: 'error'; message: string };

export interface Relay {
  id: string;
  name: string;
  rtmp_url: string;
  stream_key: string; // always "****" or "" from the API
  stream_id: string | null;
  status: RelayStatus;
  created_at: string;
  started_at: string | null;
  stopped_at: string | null;
  bytes_sent: number;
  enabled: boolean;
}

export type RelayStatus =
  | { status: 'idle' }
  | { status: 'connecting' }
  | { status: 'active' }
  | { status: 'stopped' }
  | { status: 'error'; message: string };

export interface Stats {
  active_streams: number;
  total_relays: number;
  active_relays: number;
  total_bitrate: number;
  total_bitrate_mbps: number;
  total_bytes: number;
}

export interface StreamInfo {
  stream: Stream;
  relays: Relay[];
}

export interface CreateRelayRequest {
  name: string;
  rtmp_url: string;
  stream_key: string;
}

export interface StartRelayRequest {
  stream_id: string;
}

export interface WebRTCOffer {
  sdp: string;
  type: string;
}

export interface WebRTCAnswer {
  sdp: string;
  type: string;
  session_id: string;
}

export interface IceCandidate {
  candidate: string;
  sdpMLineIndex?: number;
  sdpMid?: string;
}

export interface LogEntry {
  id: string;
  timestamp: string;
  level: 'Info' | 'Warn' | 'Error';
  message: string;
  source: string;
}

export interface Config {
  rtmp_port: number;
  relay_auto_reconnect: boolean;
  relay_reconnect_delay_secs: number;
  relay_reconnect_attempts: number;
  api_port: number;
  stun_server: string;
}
