import { useEffect, useRef } from "react"
import { useQueryClient } from "@tanstack/react-query"
import type { Stats, Stream, Relay, LogEntry } from "@/types"

const API_BASE_URL = import.meta.env.VITE_API_URL || "http://localhost:8080"

function wsUrl(): string {
  const url = new URL(`${API_BASE_URL}/api/ws`)
  url.protocol = url.protocol === "https:" ? "wss:" : "ws:"
  return url.toString()
}

type SnapshotPayload = {
  stats: Stats
  streams: Stream[]
  relays: Relay[]
}

type ServerMessage =
  | { type: "snapshot"; payload: SnapshotPayload }
  | { type: "log"; payload: LogEntry }
  | { type: "logs_init"; payload: LogEntry[] }

/**
 * One persistent WebSocket connection replacing the old per-resource polling
 * (stats/streams/relays every 1-2s, logs every 5s). The server pushes a combined
 * snapshot on an interval plus real-time log entries; this just feeds those
 * straight into the React Query cache so existing useStats/useStreams/useRelays/
 * useLogs consumers need no changes. Reconnects with backoff if the connection drops;
 * the REST-backed queryFns and a long fallback refetchInterval in each hook cover
 * the gap while disconnected.
 *
 * Mount exactly once near the app root (see App.tsx).
 */
export function useLiveSocket() {
  const queryClient = useQueryClient()
  const queryClientRef = useRef(queryClient)
  queryClientRef.current = queryClient

  useEffect(() => {
    let socket: WebSocket | null = null
    let reconnectTimer: ReturnType<typeof setTimeout> | null = null
    let attempt = 0
    let stopped = false

    const connect = () => {
      if (stopped) return
      socket = new WebSocket(wsUrl())

      socket.onopen = () => {
        attempt = 0
      }

      socket.onmessage = (event) => {
        let msg: ServerMessage
        try {
          msg = JSON.parse(event.data)
        } catch {
          return
        }
        const qc = queryClientRef.current
        switch (msg.type) {
          case "snapshot":
            qc.setQueryData(["stats"], msg.payload.stats)
            qc.setQueryData(["streams"], msg.payload.streams)
            qc.setQueryData(["relays"], msg.payload.relays)
            break
          case "logs_init":
            qc.setQueryData(["logs"], msg.payload)
            break
          case "log":
            qc.setQueryData(["logs"], (prev: LogEntry[] | undefined) => {
              const next = [...(prev ?? []), msg.payload]
              return next.length > 1000 ? next.slice(next.length - 1000) : next
            })
            break
        }
      }

      socket.onclose = () => {
        if (stopped) return
        // Exponential backoff, capped at 10s.
        const delay = Math.min(10_000, 500 * 2 ** attempt)
        attempt += 1
        reconnectTimer = setTimeout(connect, delay)
      }

      socket.onerror = () => {
        socket?.close()
      }
    }

    connect()

    return () => {
      stopped = true
      if (reconnectTimer) clearTimeout(reconnectTimer)
      socket?.close()
    }
  }, [])
}
