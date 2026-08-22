
import { useState } from "react"
import {
  Plus,
  Play,
  Power,
  Trash2,
  Share2,
  ExternalLink,
  Loader2,
} from "lucide-react"
import { Card, CardContent } from "@/components/ui/card"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Skeleton } from "@/components/ui/skeleton"
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
  DialogFooter,
  DialogDescription,
} from "@/components/ui/dialog"
import { useRelays, useCreateRelay, useDeleteRelay, useStartRelay, useStopRelay, useStreams } from "@/hooks"
import { toast } from "sonner"
import type { Relay } from "@/types"
import {  formatBytes } from "@/lib/format"
import { getRelayStatus } from "@/lib/status-utils"


function StatusBadge({ status }: { status: string }) {
  const styles =
    status === "active"
      ? "border-primary/30 bg-primary/10 text-primary"
      : status === "error"
        ? "border-destructive/30 bg-destructive/10 text-destructive"
        : status === "connecting"
          ? "border-[hsl(35,92%,55%)]/30 bg-[hsl(35,92%,55%)]/10 text-[hsl(35,92%,55%)]"
          : "border-border bg-muted text-muted-foreground"
  return (
    <Badge variant="outline" className={styles}>
      {status === "active" && <span className="mr-1.5 inline-block h-1.5 w-1.5 rounded-full bg-primary" />}
      {status}
    </Badge>
  )
}

function RelayCard({
  relay,
  onStart,
  onStop,
  onDelete,
  isStarting,
  isStopping,
  isDeleting,
}: {
  relay: Relay
  onStart: (id: string) => void
  onStop: (id: string) => void
  onDelete: (id: string) => void
  isStarting: boolean
  isStopping: boolean
  isDeleting: boolean
}) {
  const statusString = getRelayStatus(relay.status)
  const isActive = statusString === 'active'
  const canStart = statusString === 'idle' || statusString === 'stopped'
  const canStop = statusString === 'active' || statusString === 'connecting'

  return (
    <Card className="bg-card border-border">
      <CardContent className="flex flex-col gap-4 p-5">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-3">
            <h3 className="text-sm font-semibold text-foreground">{relay.name}</h3>
          </div>
          <StatusBadge status={statusString} />
        </div>

        <div className="flex flex-col gap-1.5">
          <div className="flex items-center gap-2 rounded-lg bg-muted/50 px-3 py-2">
            <span className="flex-1 truncate text-xs font-mono text-muted-foreground">{relay.rtmp_url}</span>
            <a href={relay.rtmp_url} target="_blank" rel="noopener noreferrer" className="text-muted-foreground hover:text-foreground">
              <ExternalLink className="h-3.5 w-3.5" />
              <span className="sr-only">Open URL</span>
            </a>
          </div>
          {relay.stream_key && (
            <div className="flex items-center gap-2 rounded-lg bg-muted/50 px-3 py-1.5">
              <span className="text-xs text-muted-foreground">Stream key:</span>
              <span className="text-xs font-mono text-muted-foreground">****</span>
            </div>
          )}
        </div>

        {isActive && relay.bytes_sent > 0 && (
          <div className="flex gap-4 text-xs text-muted-foreground">
            <span>Sent: <span className="text-foreground font-medium">{formatBytes(relay.bytes_sent)}</span></span>
            {relay.started_at && (
              <span>Started: <span className="text-foreground font-medium">{new Date(relay.started_at).toLocaleTimeString()}</span></span>
            )}
          </div>
        )}

        <div className="flex items-center justify-end border-t border-border pt-3">
          <div className="flex gap-2">
            {canStart && (
              <Button
                variant="outline"
                size="sm"
                className="h-8 gap-1.5 border-border bg-transparent px-3 text-muted-foreground hover:bg-accent hover:text-foreground"
                aria-label={`Start ${relay.name}`}
                onClick={() => onStart(relay.id)}
                disabled={isStarting}
              >
                {isStarting ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : <Play className="h-3.5 w-3.5" />}
                Start
              </Button>
            )}
            {canStop && (
              <Button
                variant="outline"
                size="sm"
                className="h-8 gap-1.5 border-border bg-transparent px-3 text-muted-foreground hover:bg-accent hover:text-foreground"
                aria-label={`Stop ${relay.name}`}
                onClick={() => onStop(relay.id)}
                disabled={isStopping}
              >
                {isStopping ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : <Power className="h-3.5 w-3.5" />}
                Stop
              </Button>
            )}
            <Button
              variant="outline"
              size="sm"
              className="h-8 w-8 border-border bg-transparent p-0 text-muted-foreground hover:bg-destructive/10 hover:text-destructive"
              aria-label={`Delete ${relay.name}`}
              onClick={() => onDelete(relay.id)}
              disabled={isDeleting}
            >
              {isDeleting ? <Loader2 className="h-4 w-4 animate-spin" /> : <Trash2 className="h-4 w-4" />}
            </Button>
          </div>
        </div>
      </CardContent>
    </Card>
  )
}

export function RelaysPage() {
  const [dialogOpen, setDialogOpen] = useState(false)
  const [newRelay, setNewRelay] = useState({
    name: "",
    rtmp_url: "",
    stream_key: "",
  })
  const [startingRelayId, setStartingRelayId] = useState<string | null>(null)
  const [stoppingRelayId, setStoppingRelayId] = useState<string | null>(null)
  const [deletingRelayId, setDeletingRelayId] = useState<string | null>(null)

  const { data: relays, isLoading } = useRelays()
  const { data: streams } = useStreams()
  const createRelay = useCreateRelay()
  const deleteRelay = useDeleteRelay()
  const startRelay = useStartRelay()
  const stopRelay = useStopRelay()

  const handleStart = async (id: string) => {
    const activeStream = streams?.[0]
    if (!activeStream) {
      toast.error('No active stream', {
        description: 'Please start streaming before starting a relay',
      })
      return
    }

    setStartingRelayId(id)
    try {
      await startRelay.mutateAsync({
        id,
        data: { stream_id: activeStream.id },
      })
      toast.success('Relay started successfully')
    } catch (error) {
      toast.error('Failed to start relay', {
        description: error instanceof Error ? error.message : 'Unknown error',
      })
    } finally {
      setStartingRelayId(null)
    }
  }

  const handleStop = async (id: string) => {
    setStoppingRelayId(id)
    try {
      await stopRelay.mutateAsync(id)
      toast.success('Relay stopped successfully')
    } catch (error) {
      toast.error('Failed to stop relay', {
        description: error instanceof Error ? error.message : 'Unknown error',
      })
    } finally {
      setStoppingRelayId(null)
    }
  }

  const handleDelete = async (id: string) => {
    setDeletingRelayId(id)
    try {
      await deleteRelay.mutateAsync(id)
      toast.success('Relay deleted successfully')
    } catch (error) {
      toast.error('Failed to delete relay', {
        description: error instanceof Error ? error.message : 'Unknown error',
      })
    } finally {
      setDeletingRelayId(null)
    }
  }

  const handleCreate = async () => {
    if (!newRelay.name || !newRelay.rtmp_url) {
      toast.error('Please fill in all required fields')
      return
    }

    try {
      await createRelay.mutateAsync({
        name: newRelay.name,
        rtmp_url: newRelay.rtmp_url,
        stream_key: newRelay.stream_key,
      })
      toast.success('Relay created successfully')
      setDialogOpen(false)
      setNewRelay({ name: "", rtmp_url: "", stream_key: "" })
    } catch (error) {
      toast.error('Failed to create relay', {
        description: error instanceof Error ? error.message : 'Unknown error',
      })
    }
  }

  const activeCount = relays?.filter((r) => getRelayStatus(r.status) === 'active').length || 0
  const totalCount = relays?.length || 0

  if (isLoading) {
    return (
      <div className="flex flex-col gap-6">
        <div>
          <h1 className="text-2xl font-semibold tracking-tight text-foreground">Relay Targets</h1>
          <p className="text-sm text-muted-foreground">Manage your output relay targets</p>
        </div>
        <div className="grid gap-4 md:grid-cols-2">
          <Skeleton className="h-48" />
          <Skeleton className="h-48" />
        </div>
      </div>
    )
  }

  return (
    <div className="flex flex-col gap-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-semibold tracking-tight text-foreground">Relay Targets</h1>
          <p className="text-sm text-muted-foreground">
            Manage your output relay targets ({activeCount} of {totalCount} active)
          </p>
        </div>
        <div className="flex gap-2">
          <Dialog open={dialogOpen} onOpenChange={setDialogOpen}>
            <DialogTrigger asChild>
              <Button variant="outline" size="sm" className="gap-2 border-border bg-transparent text-muted-foreground hover:bg-accent hover:text-foreground">
                <Plus className="h-4 w-4" />
                New Target
              </Button>
            </DialogTrigger>
            <DialogContent className="bg-card border-border">
              <DialogHeader>
                <DialogTitle className="text-foreground">Add Relay Target</DialogTitle>
                <DialogDescription className="text-muted-foreground">
                  Configure a new relay output destination.
                </DialogDescription>
              </DialogHeader>
              <div className="flex flex-col gap-4 py-2">
                <div className="flex flex-col gap-2">
                  <Label htmlFor="name" className="text-sm text-foreground">Name</Label>
                  <Input
                    id="name"
                    placeholder="YouTube Live"
                    value={newRelay.name}
                    onChange={(e) => setNewRelay({ ...newRelay, name: e.target.value })}
                    className="border-border bg-muted/50 text-foreground placeholder:text-muted-foreground"
                  />
                </div>
                <div className="flex flex-col gap-2">
                  <Label htmlFor="rtmp-url" className="text-sm text-foreground">RTMP Server URL</Label>
                  <Input
                    id="rtmp-url"
                    placeholder="rtmp://a.rtmp.youtube.com/live2"
                    value={newRelay.rtmp_url}
                    onChange={(e) => setNewRelay({ ...newRelay, rtmp_url: e.target.value })}
                    className="border-border bg-muted/50 font-mono text-sm text-foreground placeholder:text-muted-foreground"
                  />
                  <p className="text-xs text-muted-foreground">RTMP ingest URL without the stream key</p>
                </div>
                <div className="flex flex-col gap-2">
                  <Label htmlFor="stream-key" className="text-sm text-foreground">
                    Stream Key <span className="text-muted-foreground font-normal">(optional)</span>
                  </Label>
                  <Input
                    id="stream-key"
                    type="password"
                    placeholder="xxxx-xxxx-xxxx-xxxx"
                    value={newRelay.stream_key}
                    onChange={(e) => setNewRelay({ ...newRelay, stream_key: e.target.value })}
                    className="border-border bg-muted/50 font-mono text-sm text-foreground placeholder:text-muted-foreground"
                    autoComplete="off"
                  />
                  <p className="text-xs text-muted-foreground">Stored securely, never shown again</p>
                </div>
              </div>
              <DialogFooter>
                <Button
                  onClick={handleCreate}
                  disabled={!newRelay.name || !newRelay.rtmp_url || createRelay.isPending}
                  className="bg-primary text-primary-foreground hover:bg-primary/90"
                >
                  {createRelay.isPending && <Loader2 className="mr-2 h-4 w-4 animate-spin" />}
                  Add Target
                </Button>
              </DialogFooter>
            </DialogContent>
          </Dialog>
        </div>
      </div>

      <div className="grid gap-4 md:grid-cols-2">
        {relays?.map((relay) => (
          <RelayCard
            key={relay.id}
            relay={relay}
            onStart={handleStart}
            onStop={handleStop}
            onDelete={handleDelete}
            isStarting={startingRelayId === relay.id}
            isStopping={stoppingRelayId === relay.id}
            isDeleting={deletingRelayId === relay.id}
          />
        ))}
      </div>

      {totalCount === 0 && (
        <Card className="bg-card border-border">
          <CardContent className="flex flex-col items-center gap-3 py-12">
            <Share2 className="h-10 w-10 text-muted-foreground" />
            <p className="text-sm text-muted-foreground">No relay targets configured</p>
            <Button
              variant="outline"
              size="sm"
              className="gap-2 border-border bg-transparent text-muted-foreground hover:bg-accent hover:text-foreground"
              onClick={() => setDialogOpen(true)}
            >
              <Plus className="h-4 w-4" />
              Add your first target
            </Button>
          </CardContent>
        </Card>
      )}
    </div>
  )
}
