
import { useState } from "react"
import {
  Plus,
  Play,
  Power,
  Trash2,
  Eye,
  EyeOff,
  Share2,
  ExternalLink,
} from "lucide-react"
import { Card, CardContent } from "@/components/ui/card"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Switch } from "@/components/ui/switch"
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
  DialogFooter,
  DialogDescription,
} from "@/components/ui/dialog"
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import {
  type RelayTarget,
  type Platform,
  MOCK_RELAYS,
  maskKey,
  formatBytes,
  formatUptime,
} from "@/lib/mock-data"

function PlatformIcon({ platform }: { platform: Platform }) {
  const colors: Record<Platform, string> = {
    youtube: "bg-[hsl(0,72%,51%)] text-[hsl(0,0%,98%)]",
    twitch: "bg-[hsl(271,76%,53%)] text-[hsl(0,0%,98%)]",
    kick: "bg-[hsl(142,70%,45%)] text-[hsl(0,0%,2%)]",
    custom: "bg-muted text-muted-foreground",
  }
  return (
    <Badge className={`${colors[platform]} text-xs font-semibold border-0`}>
      {platform}
    </Badge>
  )
}

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
  onToggle,
  onDelete,
}: {
  relay: RelayTarget
  onToggle: (id: string) => void
  onDelete: (id: string) => void
}) {
  const [showKey, setShowKey] = useState(false)

  return (
    <Card className="bg-card border-border">
      <CardContent className="flex flex-col gap-4 p-5">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-3">
            <PlatformIcon platform={relay.platform} />
            <h3 className="text-sm font-semibold text-foreground">{relay.name}</h3>
          </div>
          <StatusBadge status={relay.status} />
        </div>

        <div className="flex flex-col gap-2">
          <div className="flex items-center gap-2 rounded-lg bg-muted/50 px-3 py-2">
            <span className="flex-1 truncate text-xs font-mono text-muted-foreground">{relay.url}</span>
            <a href={relay.url} target="_blank" rel="noopener noreferrer" className="text-muted-foreground hover:text-foreground">
              <ExternalLink className="h-3.5 w-3.5" />
              <span className="sr-only">Open URL</span>
            </a>
          </div>
          <div className="flex items-center gap-2 rounded-lg bg-muted/50 px-3 py-2">
            <span className="flex-1 truncate text-xs font-mono text-muted-foreground">
              {showKey ? relay.streamKey : maskKey(relay.streamKey)}
            </span>
            <button
              type="button"
              onClick={() => setShowKey(!showKey)}
              className="text-muted-foreground hover:text-foreground"
              aria-label={showKey ? "Hide stream key" : "Show stream key"}
            >
              {showKey ? <EyeOff className="h-3.5 w-3.5" /> : <Eye className="h-3.5 w-3.5" />}
            </button>
          </div>
        </div>

        {relay.status === "active" && (
          <div className="flex gap-4 text-xs text-muted-foreground">
            <span>Bitrate: <span className="text-foreground font-medium">{relay.bitrate} kbps</span></span>
            <span>Uptime: <span className="text-foreground font-medium">{formatUptime(relay.uptime)}</span></span>
            <span>Sent: <span className="text-foreground font-medium">{formatBytes(relay.bytesOut)}</span></span>
          </div>
        )}

        <div className="flex items-center justify-between border-t border-border pt-3">
          <div className="flex items-center gap-2">
            <Switch
              checked={relay.enabled}
              onCheckedChange={() => onToggle(relay.id)}
              aria-label={`Toggle ${relay.name}`}
            />
            <span className="text-xs text-muted-foreground">{relay.enabled ? "Enabled" : "Disabled"}</span>
          </div>
          <div className="flex gap-2">
            <Button
              variant="outline"
              size="sm"
              className="h-8 w-8 border-border bg-transparent p-0 text-muted-foreground hover:bg-accent hover:text-foreground"
              aria-label={`Toggle power for ${relay.name}`}
              onClick={() => onToggle(relay.id)}
            >
              <Power className="h-4 w-4" />
            </Button>
            <Button
              variant="outline"
              size="sm"
              className="h-8 w-8 border-border bg-transparent p-0 text-muted-foreground hover:bg-destructive/10 hover:text-destructive"
              aria-label={`Delete ${relay.name}`}
              onClick={() => onDelete(relay.id)}
            >
              <Trash2 className="h-4 w-4" />
            </Button>
          </div>
        </div>
      </CardContent>
    </Card>
  )
}

export function RelaysPage() {
  const [relays, setRelays] = useState<RelayTarget[]>(MOCK_RELAYS)
  const [dialogOpen, setDialogOpen] = useState(false)
  const [newRelay, setNewRelay] = useState({
    name: "",
    platform: "youtube" as Platform,
    url: "",
    streamKey: "",
  })

  const handleToggle = (id: string) => {
    setRelays((prev) =>
      prev.map((r) =>
        r.id === id
          ? {
              ...r,
              enabled: !r.enabled,
              status: r.enabled ? "inactive" : "connecting",
            }
          : r
      )
    )
  }

  const handleDelete = (id: string) => {
    setRelays((prev) => prev.filter((r) => r.id !== id))
  }

  const handleCreate = () => {
    const relay: RelayTarget = {
      id: `relay-${Date.now()}`,
      name: newRelay.name,
      platform: newRelay.platform,
      url: newRelay.url,
      streamKey: newRelay.streamKey,
      status: "inactive",
      enabled: false,
      bitrate: 0,
      uptime: 0,
      bytesOut: 0,
    }
    setRelays((prev) => [...prev, relay])
    setDialogOpen(false)
    setNewRelay({ name: "", platform: "youtube", url: "", streamKey: "" })
  }

  const handleStartAll = () => {
    setRelays((prev) =>
      prev.map((r) => ({
        ...r,
        enabled: true,
        status: r.status === "error" ? "connecting" : "active",
      }))
    )
  }

  const activeCount = relays.filter((r) => r.status === "active").length

  return (
    <div className="flex flex-col gap-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-semibold tracking-tight text-foreground">Relay Targets</h1>
          <p className="text-sm text-muted-foreground">
            Manage your output relay targets ({activeCount} of {relays.length} active)
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
                    placeholder="My Stream Relay"
                    value={newRelay.name}
                    onChange={(e) => setNewRelay({ ...newRelay, name: e.target.value })}
                    className="border-border bg-muted/50 text-foreground placeholder:text-muted-foreground"
                  />
                </div>
                <div className="flex flex-col gap-2">
                  <Label htmlFor="platform" className="text-sm text-foreground">Platform</Label>
                  <Select
                    value={newRelay.platform}
                    onValueChange={(v) => setNewRelay({ ...newRelay, platform: v as Platform })}
                  >
                    <SelectTrigger id="platform" className="border-border bg-muted/50 text-foreground">
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent className="border-border bg-card text-foreground">
                      <SelectItem value="youtube">YouTube</SelectItem>
                      <SelectItem value="twitch">Twitch</SelectItem>
                      <SelectItem value="kick">Kick</SelectItem>
                      <SelectItem value="custom">Custom</SelectItem>
                    </SelectContent>
                  </Select>
                </div>
                <div className="flex flex-col gap-2">
                  <Label htmlFor="url" className="text-sm text-foreground">RTMP URL</Label>
                  <Input
                    id="url"
                    placeholder="rtmp://..."
                    value={newRelay.url}
                    onChange={(e) => setNewRelay({ ...newRelay, url: e.target.value })}
                    className="border-border bg-muted/50 font-mono text-sm text-foreground placeholder:text-muted-foreground"
                  />
                </div>
                <div className="flex flex-col gap-2">
                  <Label htmlFor="streamKey" className="text-sm text-foreground">Stream Key</Label>
                  <Input
                    id="streamKey"
                    type="password"
                    placeholder="Your stream key"
                    value={newRelay.streamKey}
                    onChange={(e) => setNewRelay({ ...newRelay, streamKey: e.target.value })}
                    className="border-border bg-muted/50 font-mono text-sm text-foreground placeholder:text-muted-foreground"
                  />
                </div>
              </div>
              <DialogFooter>
                <Button
                  onClick={handleCreate}
                  disabled={!newRelay.name || !newRelay.url || !newRelay.streamKey}
                  className="bg-primary text-primary-foreground hover:bg-primary/90"
                >
                  Add Target
                </Button>
              </DialogFooter>
            </DialogContent>
          </Dialog>
          <Button
            size="sm"
            className="gap-2 bg-primary text-primary-foreground hover:bg-primary/90"
            onClick={handleStartAll}
          >
            <Play className="h-4 w-4" />
            Start All
          </Button>
        </div>
      </div>

      <div className="grid gap-4 md:grid-cols-2">
        {relays.map((relay) => (
          <RelayCard
            key={relay.id}
            relay={relay}
            onToggle={handleToggle}
            onDelete={handleDelete}
          />
        ))}
      </div>

      {relays.length === 0 && (
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
