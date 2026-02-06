
import { useState } from "react"
import { Save } from "lucide-react"
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Button } from "@/components/ui/button"
import { Switch } from "@/components/ui/switch"
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"

export function SettingsPage() {
  const [settings, setSettings] = useState({
    listenPort: "1935",
    listenAddr: "0.0.0.0",
    maxBitrate: "8000",
    autoReconnect: true,
    reconnectDelay: "5",
    reconnectAttempts: "10",
    logLevel: "info",
    healthCheckInterval: "300",
    enableApi: true,
    apiPort: "8080",
  })

  return (
    <div className="flex flex-col gap-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-semibold tracking-tight text-foreground">Settings</h1>
          <p className="text-sm text-muted-foreground">Configure your stream multiplexer</p>
        </div>
        <Button size="sm" className="gap-2 bg-primary text-primary-foreground hover:bg-primary/90">
          <Save className="h-4 w-4" />
          Save Changes
        </Button>
      </div>

      <div className="grid gap-6 lg:grid-cols-2">
        {/* Ingest settings */}
        <Card className="bg-card border-border">
          <CardHeader>
            <CardTitle className="text-base font-semibold text-foreground">Ingest Configuration</CardTitle>
          </CardHeader>
          <CardContent className="flex flex-col gap-4">
            <div className="flex flex-col gap-2">
              <Label htmlFor="listenAddr" className="text-sm text-foreground">Listen Address</Label>
              <Input
                id="listenAddr"
                value={settings.listenAddr}
                onChange={(e) => setSettings({ ...settings, listenAddr: e.target.value })}
                className="border-border bg-muted/50 font-mono text-sm text-foreground"
              />
            </div>
            <div className="flex flex-col gap-2">
              <Label htmlFor="listenPort" className="text-sm text-foreground">RTMP Port</Label>
              <Input
                id="listenPort"
                value={settings.listenPort}
                onChange={(e) => setSettings({ ...settings, listenPort: e.target.value })}
                className="border-border bg-muted/50 font-mono text-sm text-foreground"
              />
            </div>
            <div className="flex flex-col gap-2">
              <Label htmlFor="maxBitrate" className="text-sm text-foreground">Max Bitrate (kbps)</Label>
              <Input
                id="maxBitrate"
                value={settings.maxBitrate}
                onChange={(e) => setSettings({ ...settings, maxBitrate: e.target.value })}
                className="border-border bg-muted/50 font-mono text-sm text-foreground"
              />
            </div>
          </CardContent>
        </Card>

        {/* Relay settings */}
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
                checked={settings.autoReconnect}
                onCheckedChange={(v) => setSettings({ ...settings, autoReconnect: v })}
              />
            </div>
            <div className="flex flex-col gap-2">
              <Label htmlFor="reconnectDelay" className="text-sm text-foreground">Reconnect Delay (s)</Label>
              <Input
                id="reconnectDelay"
                value={settings.reconnectDelay}
                onChange={(e) => setSettings({ ...settings, reconnectDelay: e.target.value })}
                className="border-border bg-muted/50 font-mono text-sm text-foreground"
              />
            </div>
            <div className="flex flex-col gap-2">
              <Label htmlFor="reconnectAttempts" className="text-sm text-foreground">Max Retry Attempts</Label>
              <Input
                id="reconnectAttempts"
                value={settings.reconnectAttempts}
                onChange={(e) => setSettings({ ...settings, reconnectAttempts: e.target.value })}
                className="border-border bg-muted/50 font-mono text-sm text-foreground"
              />
            </div>
          </CardContent>
        </Card>

        {/* System settings */}
        <Card className="bg-card border-border">
          <CardHeader>
            <CardTitle className="text-base font-semibold text-foreground">System</CardTitle>
          </CardHeader>
          <CardContent className="flex flex-col gap-4">
            <div className="flex flex-col gap-2">
              <Label htmlFor="logLevel" className="text-sm text-foreground">Log Level</Label>
              <Select
                value={settings.logLevel}
                onValueChange={(v) => setSettings({ ...settings, logLevel: v })}
              >
                <SelectTrigger id="logLevel" className="border-border bg-muted/50 text-foreground">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent className="border-border bg-card text-foreground">
                  <SelectItem value="debug">Debug</SelectItem>
                  <SelectItem value="info">Info</SelectItem>
                  <SelectItem value="warn">Warning</SelectItem>
                  <SelectItem value="error">Error</SelectItem>
                </SelectContent>
              </Select>
            </div>
            <div className="flex flex-col gap-2">
              <Label htmlFor="healthCheck" className="text-sm text-foreground">Health Check Interval (s)</Label>
              <Input
                id="healthCheck"
                value={settings.healthCheckInterval}
                onChange={(e) => setSettings({ ...settings, healthCheckInterval: e.target.value })}
                className="border-border bg-muted/50 font-mono text-sm text-foreground"
              />
            </div>
          </CardContent>
        </Card>

        {/* API settings */}
        <Card className="bg-card border-border">
          <CardHeader>
            <CardTitle className="text-base font-semibold text-foreground">API</CardTitle>
          </CardHeader>
          <CardContent className="flex flex-col gap-4">
            <div className="flex items-center justify-between">
              <div className="flex flex-col gap-0.5">
                <Label className="text-sm text-foreground">Enable REST API</Label>
                <p className="text-xs text-muted-foreground">Expose a REST API for remote control</p>
              </div>
              <Switch
                checked={settings.enableApi}
                onCheckedChange={(v) => setSettings({ ...settings, enableApi: v })}
              />
            </div>
            {settings.enableApi && (
              <div className="flex flex-col gap-2">
                <Label htmlFor="apiPort" className="text-sm text-foreground">API Port</Label>
                <Input
                  id="apiPort"
                  value={settings.apiPort}
                  onChange={(e) => setSettings({ ...settings, apiPort: e.target.value })}
                  className="border-border bg-muted/50 font-mono text-sm text-foreground"
                />
              </div>
            )}
          </CardContent>
        </Card>
      </div>
    </div>
  )
}
