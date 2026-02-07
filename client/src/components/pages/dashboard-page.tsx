
import { useCallback, useMemo, useState, useEffect } from "react"
import { Activity, ArrowDown, ArrowUp, Radio, Share2, Clock, AlertTriangle, Play, Square, RotateCcw } from "lucide-react"
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { useStats, useRelays, useStreams, useStartRelay, useStopRelay } from "@/hooks"
import { formatBytes, formatUptime } from "@/lib/mock-data"
import { getRelayStatus } from "@/lib/status-utils"
import {
  ResponsiveContainer,
  AreaChart,
  Area,
  XAxis,
  YAxis,
  Tooltip,
  CartesianGrid,
} from "recharts"

function StatusDot({ status }: { status: string }) {
  const color =
    status === "active"
      ? "bg-primary"
      : status === "error"
        ? "bg-destructive"
        : status === "connecting"
          ? "bg-[hsl(35,92%,55%)]"
          : "bg-muted-foreground"
  return (
    <span className={`inline-block h-2.5 w-2.5 rounded-full ${color}`}>
      <span className="sr-only">{status}</span>
    </span>
  )
}

export function DashboardPage() {
  const [currentTime, setCurrentTime] = useState(() => Date.now())
  const { data: stats } = useStats();
  const { data: relays = [] } = useRelays();
  const { data: streams = [] } = useStreams();
  const startRelayMutation = useStartRelay();
  const stopRelayMutation = useStopRelay();

  // Update current time every second for uptime display
  useEffect(() => {
    const interval = setInterval(() => {
      setCurrentTime(Date.now())
    }, 1000)
    return () => clearInterval(interval)
  }, [])
  // console.log(relays)
  const activeStream = streams[0]; // Get the first active stream
  // const activeRelays = relays.filter((r) => 'Active' in r.status).length;
  const errorRelays = relays.filter((r) => r.status === 'Error').length;
  // const errorRelays = 0;

  // Mock bitrate history for now - in a real app, you'd track this over time
  const bitrateHistory = useMemo(() => {
    return Array.from({ length: 20 }, (_, i) => ({
      time: new Date(currentTime - (19 - i) * 5000).toLocaleTimeString(),
      bitrate: (stats?.total_bitrate || 0) / 1000, // Convert bps to kbps
    }));
  }, [stats?.total_bitrate, currentTime]);

  const startRelay = useCallback((id: string) => {
    if (!activeStream) return;
    startRelayMutation.mutate({ id, data: { stream_id: activeStream.id } });
  }, [startRelayMutation, activeStream]);

  const stopRelay = useCallback((id: string) => {
    stopRelayMutation.mutate(id);
  }, [stopRelayMutation]);

  const startAll = useCallback(() => {
    if (!activeStream) return;
    for (const relay of relays) {
      if (relay.status !== 'Active') {
        startRelayMutation.mutate({ id: relay.id, data: { stream_id: activeStream.id } });
      }
    }
  }, [relays, startRelayMutation, activeStream]);

  const stopAll = useCallback(() => {
    for (const relay of relays) {
      if (relay.status === 'Active' || relay.status === 'Connecting') {
        stopRelayMutation.mutate(relay.id);
      }
    }
  }, [relays, stopRelayMutation]);

  return (
    <div className="flex flex-col gap-6">
      <div>
        <h1 className="text-2xl font-semibold tracking-tight text-foreground">Dashboard</h1>
        <p className="text-sm text-muted-foreground">Overview of your stream multiplexer</p>
      </div>

      {/* Stats cards */}
      <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
        <Card className="bg-card border-border">
          <CardContent className="flex items-center gap-4 p-5">
            <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-lg bg-primary/10">
              <Radio className="h-5 w-5 text-primary" />
            </div>
            <div className="flex flex-col gap-0.5">
              <p className="text-xs font-medium text-muted-foreground">Stream Status</p>
              <div className="flex items-center gap-2">
                <StatusDot status={activeStream ? 'active' : 'inactive'} />
                <span className="text-lg font-semibold capitalize text-foreground">
                  {activeStream ? 'Active' : 'Inactive'}
                </span>
              </div>
            </div>
          </CardContent>
        </Card>

        <Card className="bg-card border-border">
          <CardContent className="flex items-center gap-4 p-5">
            <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-lg bg-[hsl(199,89%,48%)]/10">
              <Share2 className="h-5 w-5 text-[hsl(199,89%,48%)]" />
            </div>
            <div className="flex flex-col gap-0.5">
              <p className="text-xs font-medium text-muted-foreground">Active Relays</p>
              <div className="flex items-center gap-2">
                <span className="text-lg font-semibold text-foreground">{stats?.active_relays || 0}</span>
                <span className="text-xs text-muted-foreground">/ {stats?.total_relays || 0}</span>
              </div>
            </div>
          </CardContent>
        </Card>

        <Card className="bg-card border-border">
          <CardContent className="flex items-center gap-4 p-5">
            <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-lg bg-[hsl(35,92%,55%)]/10">
              <Clock className="h-5 w-5 text-[hsl(35,92%,55%)]" />
            </div>
            <div className="flex flex-col gap-0.5">
              <p className="text-xs font-medium text-muted-foreground">Uptime</p>
              <span className="text-lg font-semibold text-foreground">
                {activeStream
                  ? formatUptime(Math.floor((currentTime - new Date(activeStream.started_at).getTime()) / 1000))
                  : '0s'}
              </span>
            </div>
          </CardContent>
        </Card>

        <Card className="bg-card border-border">
          <CardContent className="flex items-center gap-4 p-5">
            <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-lg bg-destructive/10">
              <AlertTriangle className="h-5 w-5 text-destructive" />
            </div>
            <div className="flex flex-col gap-0.5">
              <p className="text-xs font-medium text-muted-foreground">Errors</p>
              <span className="text-lg font-semibold text-foreground">{errorRelays}</span>
            </div>
          </CardContent>
        </Card>
      </div>

      {/* Bitrate chart + Transfer stats */}
      <div className="grid gap-4 lg:grid-cols-3">
        <Card className="bg-card border-border lg:col-span-2">
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium text-muted-foreground">Bitrate (kbps)</CardTitle>
          </CardHeader>
          <CardContent className="h-64">
            <ResponsiveContainer width="100%" height="100%">
              <AreaChart data={bitrateHistory}>
                <defs>
                  <linearGradient id="bitrateGrad" x1="0" y1="0" x2="0" y2="1">
                    <stop offset="0%" stopColor="hsl(142,70%,45%)" stopOpacity={0.3} />
                    <stop offset="100%" stopColor="hsl(142,70%,45%)" stopOpacity={0} />
                  </linearGradient>
                </defs>
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
                <Area type="monotone" dataKey="bitrate" stroke="hsl(142,70%,45%)" fill="url(#bitrateGrad)" strokeWidth={2} dot={false} />
              </AreaChart>
            </ResponsiveContainer>
          </CardContent>
        </Card>

        <div className="flex flex-col gap-4">
          <Card className="bg-card border-border">
            <CardContent className="p-5">
              <div className="flex items-center gap-2 pb-3">
                <ArrowDown className="h-4 w-4 text-primary" />
                <span className="text-xs font-medium text-muted-foreground">Data In</span>
              </div>
              <p className="text-2xl font-semibold text-foreground">
                {formatBytes(activeStream?.bitrate.total_bytes || 0)}
              </p>
            </CardContent>
          </Card>
          <Card className="bg-card border-border">
            <CardContent className="p-5">
              <div className="flex items-center gap-2 pb-3">
                <ArrowUp className="h-4 w-4 text-[hsl(199,89%,48%)]" />
                <span className="text-xs font-medium text-muted-foreground">Data Out</span>
              </div>
              <p className="text-2xl font-semibold text-foreground">
                {formatBytes(relays.reduce((sum, r) => sum + r.bytes_sent, 0))}
              </p>
            </CardContent>
          </Card>
          <Card className="bg-card border-border">
            <CardContent className="p-5">
              <div className="flex items-center gap-2 pb-3">
                <Activity className="h-4 w-4 text-[hsl(35,92%,55%)]" />
                <span className="text-xs font-medium text-muted-foreground">Current Bitrate</span>
              </div>
              <p className="text-2xl font-semibold text-foreground">
                {Math.round((stats?.total_bitrate || 0) / 1000)} kbps
              </p>
            </CardContent>
          </Card>
        </div>
      </div>

      {/* Relay status + Recent errors */}
      <div className="grid gap-4 lg:grid-cols-2">
        <Card className="bg-card border-border">
          <CardHeader className="flex flex-row items-center justify-between pb-3">
            <CardTitle className="text-sm font-medium text-muted-foreground">Relay Status</CardTitle>
            <div className="flex items-center gap-2">
              <Button
                variant="outline"
                size="sm"
                className="h-7 gap-1.5 border-border bg-transparent text-xs text-foreground hover:bg-accent"
                onClick={startAll}
                disabled={!activeStream}
              >
                <Play className="h-3 w-3" />
                <span className="hidden sm:inline">Start All</span>
              </Button>
              <Button
                variant="outline"
                size="sm"
                className="h-7 gap-1.5 border-border bg-transparent text-xs text-foreground hover:bg-destructive/10 hover:text-destructive hover:border-destructive/30"
                onClick={stopAll}
              >
                <Square className="h-3 w-3" />
                <span className="hidden sm:inline">Stop All</span>
              </Button>
            </div>
          </CardHeader>
          <CardContent className="flex flex-col gap-2">
            {relays.length === 0 ? (
              <p className="text-sm text-muted-foreground py-4 text-center">No relays configured</p>
            ) : (
              relays.map((relay) => {
                const status = getRelayStatus(relay.status);
                return (
                  <div key={relay.id} className="flex items-center justify-between rounded-lg bg-muted/50 px-4 py-3">
                    <div className="flex items-center gap-3">
                      <StatusDot status={status} />
                      <span className="text-sm font-medium text-foreground">{relay.name}</span>
                      <Badge
                        variant="outline"
                        className={
                          status === "active"
                            ? "border-primary/30 bg-primary/10 text-primary text-[10px]"
                            : status === "error"
                              ? "border-destructive/30 bg-destructive/10 text-destructive text-[10px]"
                              : status === "connecting"
                                ? "border-[hsl(35,92%,55%)]/30 bg-[hsl(35,92%,55%)]/10 text-[hsl(35,92%,55%)] text-[10px]"
                                : "border-border bg-muted text-muted-foreground text-[10px]"
                        }
                      >
                        {status}
                      </Badge>
                    </div>
                    <div className="flex items-center gap-1.5">
                      {(status === "active" || status === "connecting") ? (
                        <Button
                          variant="ghost"
                          size="icon"
                          className="h-7 w-7 text-muted-foreground hover:text-destructive hover:bg-destructive/10"
                          onClick={() => stopRelay(relay.id)}
                          aria-label={`Stop ${relay.name}`}
                        >
                          <Square className="h-3.5 w-3.5" />
                        </Button>
                      ) : (
                        <Button
                          variant="ghost"
                          size="icon"
                          className="h-7 w-7 text-muted-foreground hover:text-primary hover:bg-primary/10"
                          onClick={() => startRelay(relay.id)}
                          aria-label={`Start ${relay.name}`}
                          disabled={!activeStream}
                        >
                          <Play className="h-3.5 w-3.5" />
                        </Button>
                      )}
                      {status === "error" && (
                        <Button
                          variant="ghost"
                          size="icon"
                          className="h-7 w-7 text-muted-foreground hover:text-[hsl(35,92%,55%)] hover:bg-[hsl(35,92%,55%)]/10"
                          onClick={() => startRelay(relay.id)}
                          aria-label={`Retry ${relay.name}`}
                          disabled={!activeStream}
                        >
                          <RotateCcw className="h-3.5 w-3.5" />
                        </Button>
                      )}
                    </div>
                  </div>
                );
              })
            )}
          </CardContent>
        </Card>

        <Card className="bg-card border-border">
          <CardHeader className="pb-3">
            <CardTitle className="text-sm font-medium text-muted-foreground">Recent Errors</CardTitle>
          </CardHeader>
          <CardContent className="flex flex-col gap-3">
            {errorRelays === 0 ? (
              <p className="text-sm text-muted-foreground">No recent errors</p>
            ) : (
              relays
                .filter(r => r.status === 'Error')
                .map((relay) => (
                  <div key={relay.id} className="flex flex-col gap-1 rounded-lg bg-destructive/5 px-4 py-3">
                    <div className="flex items-center justify-between">
                      <Badge variant="outline" className="border-destructive/30 bg-destructive/10 text-destructive text-xs">
                        {relay.name}
                      </Badge>
                      <span className="text-xs text-muted-foreground font-mono">
                        {relay.stopped_at ? new Date(relay.stopped_at).toLocaleTimeString() : 'Now'}
                      </span>
                    </div>
                    <p className="text-sm text-foreground">
                      {relay.status === 'Error' ? 'Error' : 'Unknown error'}
                    </p>
                  </div>
                ))
            )}
          </CardContent>
        </Card>
      </div>
    </div>
  );
}