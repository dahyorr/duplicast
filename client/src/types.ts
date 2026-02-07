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

export type StreamStatus = 'Active' | 'Inactive' | 'Error';

export interface Relay {
  id: string;
  name: string;
  target_url: string;
  stream_id: string | null;
  status: RelayStatus;
  created_at: string;
  started_at: string | null;
  stopped_at: string | null;
  bytes_sent: number;
  enabled: boolean;
}

export type RelayStatus = 'Idle' | 'Connecting' | 'Active' | 'Stopped' | 'Error';

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
  target_url: string;
}

export interface StartRelayRequest {
  stream_id: string;
}
