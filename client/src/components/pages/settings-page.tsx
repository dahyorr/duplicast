
import { useEffect, useState } from "react"
import { Save, CheckCircle2, XCircle } from "lucide-react"
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Button } from "@/components/ui/button"
import { Switch } from "@/components/ui/switch"
import { useConfig, useSaveConfig } from "@/hooks"
import type { Config } from "@/types"

// Local form state keeps numeric fields as strings so HTML inputs stay controlled.
type FormState = {
  rtmp_port: string
  relay_auto_reconnect: boolean
  relay_reconnect_delay_secs: string
  relay_reconnect_attempts: string
  api_port: string
  stun_server: string
}

function configToForm(c: Config): FormState {
  return {
    rtmp_port: String(c.rtmp_port),
    relay_auto_reconnect: c.relay_auto_reconnect,
    relay_reconnect_delay_secs: String(c.relay_reconnect_delay_secs),
    relay_reconnect_attempts: String(c.relay_reconnect_attempts),
    api_port: String(c.api_port),
    stun_server: c.stun_server,
  }
}

function formToConfig(f: FormState): Config {
  return {
    rtmp_port: parseInt(f.rtmp_port) || 1935,
    relay_auto_reconnect: f.relay_auto_reconnect,
    relay_reconnect_delay_secs: parseInt(f.relay_reconnect_delay_secs) || 5,
    relay_reconnect_attempts: parseInt(f.relay_reconnect_attempts) || 10,
    api_port: parseInt(f.api_port) || 8080,
    stun_server: f.stun_server || "stun.l.google.com:19302",
  }
}

export function SettingsPage() {
  const { data: config, isLoading } = useConfig()
  const saveConfig = useSaveConfig()

  const [form, setForm] = useState<FormState>({
    rtmp_port: "1935",
    relay_auto_reconnect: true,
    relay_reconnect_delay_secs: "5",
    relay_reconnect_attempts: "10",
    api_port: "8080",
    stun_server: "stun.l.google.com:19302",
  })

  useEffect(() => {
    if (config) setForm(configToForm(config))
  }, [config])

  const set = (patch: Partial<FormState>) => setForm((f) => ({ ...f, ...patch }))

  const inputClass = "border-border bg-muted/50 font-mono text-sm text-foreground"

  return (
    <div className="flex flex-col gap-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-semibold tracking-tight text-foreground">Settings</h1>
          <p className="text-sm text-muted-foreground">Configure your stream multiplexer</p>
        </div>

        <div className="flex items-center gap-3">
          {saveConfig.isSuccess && (
            <span className="flex items-center gap-1.5 text-sm text-green-500">
              <CheckCircle2 className="h-4 w-4" /> Saved
            </span>
          )}
          {saveConfig.isError && (
            <span className="flex items-center gap-1.5 text-sm text-destructive">
              <XCircle className="h-4 w-4" /> Save failed
            </span>
          )}
          <Button
            size="sm"
            className="gap-2 bg-primary text-primary-foreground hover:bg-primary/90"
            onClick={() => saveConfig.mutate(formToConfig(form))}
            disabled={isLoading || saveConfig.isPending}
          >
            <Save className="h-4 w-4" />
            {saveConfig.isPending ? "Saving…" : "Save Changes"}
          </Button>
        </div>
      </div>

      <div className="grid gap-6 lg:grid-cols-2">
        {/* Ingest */}
        <Card className="bg-card border-border">
          <CardHeader>
            <CardTitle className="text-base font-semibold text-foreground">Ingest Configuration</CardTitle>
          </CardHeader>
          <CardContent className="flex flex-col gap-4">
            <div className="flex flex-col gap-2">
              <Label htmlFor="listenPort" className="text-sm text-foreground">RTMP Port</Label>
              <Input
                id="listenPort"
                value={form.rtmp_port}
                onChange={(e) => set({ rtmp_port: e.target.value })}
                className={inputClass}
              />
            </div>
          </CardContent>
        </Card>

        {/* Relay */}
        <Card className="bg-card border-border">
          <CardHeader>
            <CardTitle className="text-base font-semibold text-foreground">Relay Configuration</CardTitle>
          </CardHeader>
          <CardContent className="flex flex-col gap-4">
            <div className="flex items-center justify-between">
              <div className="flex flex-col gap-0.5">
                <Label className="text-sm text-foreground">Auto Reconnect</Label>
                <p className="text-xs text-muted-foreground">Automatically reconnect failed relays</p>
              </div>
              <Switch
                checked={form.relay_auto_reconnect}
                onCheckedChange={(v) => set({ relay_auto_reconnect: v })}
              />
            </div>
            <div className="flex flex-col gap-2">
              <Label htmlFor="reconnectDelay" className="text-sm text-foreground">Reconnect Delay (s)</Label>
              <Input
                id="reconnectDelay"
                value={form.relay_reconnect_delay_secs}
                onChange={(e) => set({ relay_reconnect_delay_secs: e.target.value })}
                className={inputClass}
              />
            </div>
            <div className="flex flex-col gap-2">
              <Label htmlFor="reconnectAttempts" className="text-sm text-foreground">Max Retry Attempts</Label>
              <Input
                id="reconnectAttempts"
                value={form.relay_reconnect_attempts}
                onChange={(e) => set({ relay_reconnect_attempts: e.target.value })}
                className={inputClass}
              />
            </div>
          </CardContent>
        </Card>

        {/* API */}
        <Card className="bg-card border-border">
          <CardHeader>
            <CardTitle className="text-base font-semibold text-foreground">API</CardTitle>
          </CardHeader>
          <CardContent className="flex flex-col gap-4">
            <div className="flex flex-col gap-2">
              <Label htmlFor="apiPort" className="text-sm text-foreground">API Port</Label>
              <Input
                id="apiPort"
                value={form.api_port}
                onChange={(e) => set({ api_port: e.target.value })}
                className={inputClass}
              />
            </div>
          </CardContent>
        </Card>

        {/* WebRTC */}
        <Card className="bg-card border-border">
          <CardHeader>
            <CardTitle className="text-base font-semibold text-foreground">WebRTC Preview</CardTitle>
          </CardHeader>
          <CardContent className="flex flex-col gap-4">
            <div className="flex flex-col gap-2">
              <Label htmlFor="stunServer" className="text-sm text-foreground">STUN Server (host:port)</Label>
              <Input
                id="stunServer"
                value={form.stun_server}
                onChange={(e) => set({ stun_server: e.target.value })}
                className={inputClass}
              />
            </div>
          </CardContent>
        </Card>
      </div>
    </div>
  )
}
