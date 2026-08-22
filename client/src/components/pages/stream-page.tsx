
import { useState, useMemo, useEffect } from "react"
import { Copy, Check, Gauge, Film, Clock, ArrowDown, RefreshCw, AlertCircle } from "lucide-react"
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { useStreams } from "@/hooks"
import { formatBytes, formatUptime, formatBitrate } from "@/lib/format"
import { getStreamStatus } from "@/lib/status-utils"
import { Alert, AlertDescription } from "@/components/ui/alert"
import { Skeleton } from "@/components/ui/skeleton"
import { StreamPreview } from "@/components/StreamPreview"
import {
  ResponsiveContainer,
  LineChart,
  Line,
  XAxis,
  YAxis,
  Tooltip,
  CartesianGrid,
} from "recharts"

function StatusDot({ status }: { status: string }) {
  const color =
    status === "active"
      ? "bg-[hsl(142,70%,45%)]"
      : status === "error"
        ? "bg-[hsl(0,72%,51%)]"
        : "bg-muted-foreground"
  return <span className={`inline-block h-2.5 w-2.5 rounded-full ${color}`}><span className="sr-only">{status}</span></span>
}

export function StreamPage() {
  const [copied, setCopied] = useState(false)
  const [currentTime, setCurrentTime] = useState(() => Date.now())
  const { data: streams, isLoading, error, refetch } = useStreams()

  // Update current time every second for uptime display
  useEffect(() => {
    const interval = setInterval(() => {
      setCurrentTime(Date.now())
    }, 1000)
    return () => clearInterval(interval)
  }, [])

  // Get the first (and likely only) active stream
  const stream = streams?.[0]

  // Calculate uptime and bitrate history
  const uptime = useMemo(() => {
    if (!stream?.started_at) return 0
    const startTime = new Date(stream.started_at).getTime()
    return Math.floor((currentTime - startTime) / 1000)
  }, [stream, currentTime])

  const bitrateHistory = useMemo(() => {
    const currentBitrate = (stream?.bitrate?.total_bitrate || 0) / 1_000_000 // Convert bps to Mbps
    return Array.from({ length: 20 }, (_, i) => ({
      time: new Date(Date.now() - (19 - i) * 5000).toLocaleTimeString(),
      ingest: Number.parseFloat(currentBitrate.toFixed(2)),
    }))
  }, [stream?.bitrate?.total_bitrate])

  const handleCopy = () => {
    if (stream) {
      const streamUrl = `rtmp://localhost:1935/${stream.app_name}/${stream.stream_key}`
      navigator.clipboard.writeText(streamUrl)
      setCopied(true)
      setTimeout(() => setCopied(false), 2000)
    }
  }

  if (isLoading) {
    return (
      <div className="flex flex-col gap-6">
        <div className="flex items-center justify-between">
          <div>
            <h1 className="text-2xl font-semibold tracking-tight text-foreground">Stream</h1>
            <p className="text-sm text-muted-foreground">Monitor and manage your ingest stream</p>
          </div>
        </div>
        <Skeleton className="h-40 w-full" />
        <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
          <Skeleton className="h-24" />
          <Skeleton className="h-24" />
          <Skeleton className="h-24" />
          <Skeleton className="h-24" />
        </div>
      </div>
    )
  }

  if (error) {
    return (
      <div className="flex flex-col gap-6">
        <div>
          <h1 className="text-2xl font-semibold tracking-tight text-foreground">Stream</h1>
          <p className="text-sm text-muted-foreground">Monitor and manage your ingest stream</p>
        </div>
        <Alert variant="destructive">
          <AlertCircle className="h-4 w-4" />
          <AlertDescription>
            Failed to load stream data. {error instanceof Error ? error.message : 'Unknown error'}
          </AlertDescription>
        </Alert>
      </div>
    )
  }

  if (!stream) {
    return (
      <div className="flex flex-col gap-6">
        <div>
          <h1 className="text-2xl font-semibold tracking-tight text-foreground">Stream</h1>
          <p className="text-sm text-muted-foreground">Monitor and manage your ingest stream</p>
        </div>
        <Alert>
          <AlertCircle className="h-4 w-4" />
          <AlertDescription>
            No active stream. Start streaming to <code>rtmp://localhost:1935/live</code>
          </AlertDescription>
        </Alert>
      </div>
    )
  }

  const statusString = getStreamStatus(stream.status)
  const streamUrl = `rtmp://localhost:1935/${stream.app_name}/${stream.stream_key}`

  return (
    <div className="flex flex-col gap-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-semibold tracking-tight text-foreground">Stream</h1>
          <p className="text-sm text-muted-foreground">Monitor and manage your ingest stream</p>
        </div>
        <Button variant="outline" size="sm" className="gap-2 border-border bg-transparent text-muted-foreground hover:bg-accent hover:text-foreground" onClick={() => refetch()}>
          <RefreshCw className="h-4 w-4" />
          Refresh
        </Button>
      </div>

      {/* Stream URL + Status */}
      <div className="grid gap-4 lg:grid-cols-5">
        <Card className="bg-card border-border lg:col-span-3">
          <CardContent className="flex flex-col gap-5 p-5">
            <div className="flex flex-col gap-2">
              <label className="text-xs font-medium text-muted-foreground">Stream URL</label>
              <div className="flex items-center gap-2 rounded-lg border border-border bg-muted/50 px-4 py-2.5">
                <code className="flex-1 text-sm font-mono text-foreground truncate">{'$ '}{streamUrl}</code>
                <button
                  type="button"
                  onClick={handleCopy}
                  className="rounded p-1 text-muted-foreground transition-colors hover:text-foreground"
                  aria-label="Copy stream URL"
                >
                  {copied ? <Check className="h-4 w-4 text-primary" /> : <Copy className="h-4 w-4" />}
                </button>
              </div>
            </div>

            <div className="flex items-center gap-3">
              <span className="text-sm text-muted-foreground">Stream Status:</span>
              <div className="flex items-center gap-2">
                <StatusDot status={statusString} />
                <span className="text-sm font-medium capitalize text-foreground">{statusString}</span>
              </div>
              <Badge
                variant="outline"
                className="ml-auto border-primary/30 bg-primary/10 text-primary"
              >
                Live Stream
              </Badge>
            </div>
          </CardContent>
        </Card>

        <StreamPreview
          streamUrl={statusString === "active" ? streamUrl : undefined}
          streamId={stream.id}
          className="lg:col-span-2"
          autoPlay={true}
        />
      </div>

      {/* Stream details */}
      <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
        <Card className="bg-card border-border">
          <CardContent className="flex items-center gap-4 p-5">
            <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-lg bg-primary/10">
              <Gauge className="h-5 w-5 text-primary" />
            </div>
            <div>
              <p className="text-xs text-muted-foreground">Bitrate</p>
              <p className="text-lg font-semibold text-foreground">
                {formatBitrate(stream.bitrate?.total_bitrate || 0)}
              </p>
            </div>
          </CardContent>
        </Card>
        <Card className="bg-card border-border">
          <CardContent className="flex items-center gap-4 p-5">
            <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-lg bg-[hsl(199,89%,48%)]/10">
              <Film className="h-5 w-5 text-[hsl(199,89%,48%)]" />
            </div>
            <div>
              <p className="text-xs text-muted-foreground">Publisher</p>
              <p className="text-lg font-semibold text-foreground truncate">{stream.publisher_addr}</p>
            </div>
          </CardContent>
        </Card>
        <Card className="bg-card border-border">
          <CardContent className="flex items-center gap-4 p-5">
            <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-lg bg-[hsl(35,92%,55%)]/10">
              <Clock className="h-5 w-5 text-[hsl(35,92%,55%)]" />
            </div>
            <div>
              <p className="text-xs text-muted-foreground">Uptime</p>
              <p className="text-lg font-semibold text-foreground">{formatUptime(uptime)}</p>
            </div>
          </CardContent>
        </Card>
        <Card className="bg-card border-border">
          <CardContent className="flex items-center gap-4 p-5">
            <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-lg bg-[hsl(271,76%,53%)]/10">
              <ArrowDown className="h-5 w-5 text-[hsl(271,76%,53%)]" />
            </div>
            <div>
              <p className="text-xs text-muted-foreground">Data Received</p>
              <p className="text-lg font-semibold text-foreground">{formatBytes(stream.bitrate?.total_bytes || 0)}</p>
            </div>
          </CardContent>
        </Card>
      </div>

      {/* Bitrate chart */}
      <Card className="bg-card border-border">
        <CardHeader className="pb-2">
          <CardTitle className="text-sm font-medium text-muted-foreground">Ingest Bitrate History (kbps)</CardTitle>
        </CardHeader>
        <CardContent className="h-72">
          <ResponsiveContainer width="100%" height="100%">
            <LineChart data={bitrateHistory}>
              <CartesianGrid strokeDasharray="3 3" stroke="hsl(0,0%,12%)" />
              <XAxis dataKey="time" tick={{ fill: "hsl(0,0%,45%)", fontSize: 11 }} tickLine={false} axisLine={false} />
              <YAxis tick={{ fill: "hsl(0,0%,45%)", fontSize: 11 }} tickLine={false} axisLine={false} />
              <Tooltip
                contentStyle={{
                  backgroundColor: "hsl(0,0%,6%)",
                  border: "1px solid hsl(0,0%,12%)",
                  borderRadius: "8px",
                  color: "hsl(0,0%,93%)",
                  fontSize: 12,
                }}
              />
              <Line type="monotone" dataKey="ingest" stroke="hsl(142,70%,45%)" strokeWidth={2} dot={false} />
            </LineChart>
          </ResponsiveContainer>
        </CardContent>
      </Card>
    </div>
  )
}
