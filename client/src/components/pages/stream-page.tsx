
import { useState } from "react"
import { Copy, Check, Tv, Gauge, Film, Clock, ArrowDown, RefreshCw } from "lucide-react"
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { MOCK_STREAM, formatBytes, formatUptime, BITRATE_HISTORY } from "@/lib/mock-data"
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
  const stream = MOCK_STREAM

  const handleCopy = () => {
    navigator.clipboard.writeText(stream.url)
    setCopied(true)
    setTimeout(() => setCopied(false), 2000)
  }

  return (
    <div className="flex flex-col gap-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-semibold tracking-tight text-foreground">Stream</h1>
          <p className="text-sm text-muted-foreground">Monitor and manage your ingest stream</p>
        </div>
        <Button variant="outline" size="sm" className="gap-2 border-border bg-transparent text-muted-foreground hover:bg-accent hover:text-foreground">
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
                <code className="flex-1 text-sm font-mono text-foreground">{'$ '}{stream.url}</code>
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
                <StatusDot status={stream.status} />
                <span className="text-sm font-medium capitalize text-foreground">{stream.status}</span>
              </div>
              <Badge
                variant="outline"
                className="ml-auto border-primary/30 bg-primary/10 text-primary"
              >
                {stream.resolution} @ {stream.fps}fps
              </Badge>
            </div>
          </CardContent>
        </Card>

        <Card className="bg-card border-border lg:col-span-2 overflow-hidden">
          <CardContent className="relative flex h-full min-h-[160px] items-center justify-center bg-muted/30 p-0">
            <div className="flex flex-col items-center gap-2 text-muted-foreground">
              <Tv className="h-10 w-10" />
              <span className="text-xs">Stream Preview</span>
              {stream.status === "active" && (
                <span className="flex items-center gap-1.5 text-xs text-primary">
                  <span className="inline-block h-1.5 w-1.5 animate-pulse rounded-full bg-primary" />
                  LIVE
                </span>
              )}
            </div>
          </CardContent>
        </Card>
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
              <p className="text-lg font-semibold text-foreground">{stream.bitrate} kbps</p>
            </div>
          </CardContent>
        </Card>
        <Card className="bg-card border-border">
          <CardContent className="flex items-center gap-4 p-5">
            <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-lg bg-[hsl(199,89%,48%)]/10">
              <Film className="h-5 w-5 text-[hsl(199,89%,48%)]" />
            </div>
            <div>
              <p className="text-xs text-muted-foreground">Codec</p>
              <p className="text-lg font-semibold text-foreground">{stream.codec}</p>
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
              <p className="text-lg font-semibold text-foreground">{formatUptime(stream.uptime)}</p>
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
              <p className="text-lg font-semibold text-foreground">{formatBytes(stream.bytesIn)}</p>
            </div>
          </CardContent>
        </Card>
      </div>

      {/* Bitrate chart */}
      <Card className="bg-card border-border">
        <CardHeader className="pb-2">
          <CardTitle className="text-sm font-medium text-muted-foreground">Ingest Bitrate History</CardTitle>
        </CardHeader>
        <CardContent className="h-72">
          <ResponsiveContainer width="100%" height="100%">
            <LineChart data={BITRATE_HISTORY}>
              <CartesianGrid strokeDasharray="3 3" stroke="hsl(0,0%,12%)" />
              <XAxis dataKey="time" tick={{ fill: "hsl(0,0%,45%)", fontSize: 11 }} tickLine={false} axisLine={false} />
              <YAxis tick={{ fill: "hsl(0,0%,45%)", fontSize: 11 }} tickLine={false} axisLine={false} domain={[5000, 7000]} />
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
