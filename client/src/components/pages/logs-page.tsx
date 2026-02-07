
import { useState } from "react"
import { Search, Filter, AlertTriangle, Info, AlertCircle } from "lucide-react"
import { Card, CardContent } from "@/components/ui/card"
import { Badge } from "@/components/ui/badge"
import { Input } from "@/components/ui/input"
import { Button } from "@/components/ui/button"
import { Alert, AlertDescription } from "@/components/ui/alert"
import { MOCK_LOGS, type LogEntry } from "@/lib/mock-data"

function LogIcon({ level }: { level: LogEntry["level"] }) {
  if (level === "error") return <AlertCircle className="h-4 w-4 text-destructive" />
  if (level === "warn") return <AlertTriangle className="h-4 w-4 text-[hsl(35,92%,55%)]" />
  return <Info className="h-4 w-4 text-[hsl(199,89%,48%)]" />
}

function LevelBadge({ level }: { level: LogEntry["level"] }) {
  const styles =
    level === "error"
      ? "border-destructive/30 bg-destructive/10 text-destructive"
      : level === "warn"
        ? "border-[hsl(35,92%,55%)]/30 bg-[hsl(35,92%,55%)]/10 text-[hsl(35,92%,55%)]"
        : "border-[hsl(199,89%,48%)]/30 bg-[hsl(199,89%,48%)]/10 text-[hsl(199,89%,48%)]"
  return (
    <Badge variant="outline" className={`${styles} w-14 justify-center text-xs`}>
      {level}
    </Badge>
  )
}

export function LogsPage() {
  const [filter, setFilter] = useState<"all" | "info" | "warn" | "error">("all")
  const [search, setSearch] = useState("")

  const filteredLogs = MOCK_LOGS.filter((log) => {
    const matchesLevel = filter === "all" || log.level === filter
    const matchesSearch =
      search === "" ||
      log.message.toLowerCase().includes(search.toLowerCase()) ||
      log.source.toLowerCase().includes(search.toLowerCase())
    return matchesLevel && matchesSearch
  }).reverse()

  return (
    <div className="flex flex-col gap-6">
      <div>
        <h1 className="text-2xl font-semibold tracking-tight text-foreground">Logs</h1>
        <p className="text-sm text-muted-foreground">System and relay event logs</p>
      </div>

      <Alert>
        <AlertCircle className="h-4 w-4" />
        <AlertDescription>
          Currently showing mock data. Backend logs API endpoint is not yet implemented.
        </AlertDescription>
      </Alert>

      <div className="flex flex-col gap-3 sm:flex-row sm:items-center">
        <div className="relative flex-1">
          <Search className="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
          <Input
            placeholder="Search logs..."
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            className="border-border bg-muted/50 pl-9 text-foreground placeholder:text-muted-foreground"
          />
        </div>
        <div className="flex gap-1">
          {(["all", "info", "warn", "error"] as const).map((level) => (
            <Button
              key={level}
              variant="outline"
              size="sm"
              onClick={() => setFilter(level)}
              className={`border-border bg-transparent capitalize ${filter === level
                  ? "bg-accent text-foreground"
                  : "text-muted-foreground hover:bg-accent/50 hover:text-foreground"
                }`}
            >
              {level}
            </Button>
          ))}
        </div>
      </div>

      <Card className="bg-card border-border">
        <CardContent className="p-0">
          <div className="flex flex-col divide-y divide-border">
            {filteredLogs.length === 0 ? (
              <div className="flex flex-col items-center gap-2 py-12">
                <Filter className="h-8 w-8 text-muted-foreground" />
                <p className="text-sm text-muted-foreground">No logs match your filter</p>
              </div>
            ) : (
              filteredLogs.map((log) => (
                <div
                  key={log.id}
                  className="flex items-start gap-3 px-4 py-3 transition-colors hover:bg-muted/30"
                >
                  <LogIcon level={log.level} />
                  <div className="flex flex-1 flex-col gap-1 sm:flex-row sm:items-center sm:gap-3">
                    <span className="shrink-0 text-xs font-mono text-muted-foreground">
                      {new Date(log.timestamp).toLocaleTimeString()}
                    </span>
                    <LevelBadge level={log.level} />
                    <Badge variant="outline" className="w-fit border-border bg-muted text-muted-foreground text-xs">
                      {log.source}
                    </Badge>
                    <span className="text-sm text-foreground">{log.message}</span>
                  </div>
                </div>
              ))
            )}
          </div>
        </CardContent>
      </Card>
    </div>
  )
}
