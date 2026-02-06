export type StreamStatus = "active" | "inactive" | "error"
export type RelayStatus = "active" | "inactive" | "connecting" | "error"
export type Platform = "youtube" | "twitch" | "kick" | "custom"

export interface StreamInfo {
  id: string
  url: string
  status: StreamStatus
  bitrate: number
  fps: number
  resolution: string
  codec: string
  uptime: number
  viewers: number
  bytesIn: number
  bytesOut: number
}

export interface RelayTarget {
  id: string
  name: string
  platform: Platform
  url: string
  streamKey: string
  status: RelayStatus
  enabled: boolean
  bitrate: number
  uptime: number
  bytesOut: number
}

export interface LogEntry {
  id: string
  timestamp: string
  level: "info" | "warn" | "error"
  message: string
  source: string
}

export const MOCK_STREAM: StreamInfo = {
  id: "stream-1",
  url: "rtmp://localhost:1935",
  status: "active",
  bitrate: 6000,
  fps: 60,
  resolution: "1920x1080",
  codec: "H.264 / AAC",
  uptime: 7245,
  viewers: 0,
  bytesIn: 2_147_483_648,
  bytesOut: 8_589_934_592,
}

export const MOCK_RELAYS: RelayTarget[] = [
  {
    id: "relay-1",
    name: "YouTube Live",
    platform: "youtube",
    url: "rtmp://a.rtmp.youtube.com/live2",
    streamKey: "xxxx-xxxx-xxxx-6cpr",
    status: "active",
    enabled: true,
    bitrate: 6000,
    uptime: 7200,
    bytesOut: 2_147_483_648,
  },
  {
    id: "relay-2",
    name: "Twitch",
    platform: "twitch",
    url: "rtmp://live.twitch.tv/app",
    streamKey: "live_xxxxxxxx_xxxxDk4f",
    status: "active",
    enabled: true,
    bitrate: 6000,
    uptime: 7200,
    bytesOut: 2_147_483_648,
  },
  {
    id: "relay-3",
    name: "Kick",
    platform: "kick",
    url: "rtmp://fa723fc1b171.global-contribute.live-video.net/app",
    streamKey: "sk_us-east-1_xxxx_xxxxRt7p",
    status: "inactive",
    enabled: false,
    bitrate: 0,
    uptime: 0,
    bytesOut: 0,
  },
  {
    id: "relay-4",
    name: "Backup Server",
    platform: "custom",
    url: "rtmp://backup.example.com/live",
    streamKey: "backup-stream-key-xxxx",
    status: "error",
    enabled: true,
    bitrate: 0,
    uptime: 0,
    bytesOut: 0,
  },
]

export const MOCK_LOGS: LogEntry[] = [
  { id: "log-1", timestamp: "2026-02-06T14:32:01Z", level: "info", message: "Stream connected from 192.168.1.100", source: "ingest" },
  { id: "log-2", timestamp: "2026-02-06T14:32:02Z", level: "info", message: "Relay started: YouTube Live", source: "relay" },
  { id: "log-3", timestamp: "2026-02-06T14:32:02Z", level: "info", message: "Relay started: Twitch", source: "relay" },
  { id: "log-4", timestamp: "2026-02-06T14:35:15Z", level: "warn", message: "Bitrate drop detected: 6000 -> 4200 kbps", source: "ingest" },
  { id: "log-5", timestamp: "2026-02-06T14:35:18Z", level: "info", message: "Bitrate recovered: 6000 kbps", source: "ingest" },
  { id: "log-6", timestamp: "2026-02-06T14:40:30Z", level: "error", message: "Relay connection failed: Backup Server - Connection refused", source: "relay" },
  { id: "log-7", timestamp: "2026-02-06T14:40:31Z", level: "info", message: "Retry scheduled for Backup Server in 5s", source: "relay" },
  { id: "log-8", timestamp: "2026-02-06T14:40:36Z", level: "error", message: "Relay retry failed: Backup Server - Connection refused", source: "relay" },
  { id: "log-9", timestamp: "2026-02-06T14:45:00Z", level: "info", message: "Health check passed: all systems nominal", source: "system" },
  { id: "log-10", timestamp: "2026-02-06T14:50:00Z", level: "info", message: "Health check passed: all systems nominal", source: "system" },
]

export const BITRATE_HISTORY = Array.from({ length: 60 }, (_, i) => ({
  time: `${i}s`,
  ingest: 5800 + Math.random() * 400,
  youtube: 5700 + Math.random() * 500,
  twitch: 5600 + Math.random() * 600,
}))

export function formatBytes(bytes: number): string {
  if (bytes === 0) return "0 B"
  const k = 1024
  const sizes = ["B", "KB", "MB", "GB", "TB"]
  const i = Math.floor(Math.log(bytes) / Math.log(k))
  return `${Number.parseFloat((bytes / k ** i).toFixed(2))} ${sizes[i]}`
}

export function formatUptime(seconds: number): string {
  if (seconds === 0) return "0s"
  const h = Math.floor(seconds / 3600)
  const m = Math.floor((seconds % 3600) / 60)
  const s = seconds % 60
  const parts = []
  if (h > 0) parts.push(`${h}h`)
  if (m > 0) parts.push(`${m}m`)
  if (s > 0) parts.push(`${s}s`)
  return parts.join(" ")
}

export function maskKey(key: string): string {
  if (key.length <= 8) return "****"
  return `${"*".repeat(key.length - 4)}${key.slice(-4)}`
}
