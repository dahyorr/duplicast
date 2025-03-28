export enum AppStateEvents {
  ServersReady = "servers-ready",
  StreamPreviewActive = "stream-preview-active",
  StreamActive = "stream-active",
  StreamPreviewEnded = "stream-preview-ended",
  StreamPreviewFailed = "stream-preview-failed",
  StreamEnded = "stream-ended",
}

export interface RelayTarget {
  id: string;
  tag: string;
  stream_key: string;
  created_at: string;
  url: string;
  enabled: boolean;
}