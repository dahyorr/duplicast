
import {
  LayoutDashboard,
  Radio,
  Share2,
  Settings,
  Github,
  Activity,
} from "lucide-react"
import { cn } from "@/lib/utils"

interface AppSidebarProps {
  currentPage: string
  onNavigate: (page: string) => void
}

const navItems = [
  { id: "dashboard", label: "Dashboard", icon: LayoutDashboard },
  { id: "stream", label: "Stream", icon: Radio },
  { id: "relays", label: "Relays", icon: Share2 },
  { id: "logs", label: "Logs", icon: Activity },
  { id: "settings", label: "Settings", icon: Settings },
]

export function AppSidebar({ currentPage, onNavigate }: AppSidebarProps) {
  return (
    <aside className="flex h-screen w-16 flex-col items-center border-r border-border bg-card py-4 lg:w-56 lg:items-stretch">
      <div className="flex items-center justify-center gap-2 px-4 pb-6">
        <div className="flex h-8 w-8 items-center justify-center rounded-lg bg-primary">
          <Radio className="h-4 w-4 text-primary-foreground" />
        </div>
        <span className="hidden text-lg font-semibold tracking-tight text-foreground lg:block">
          Duplicast
        </span>
      </div>

      <nav className="flex flex-1 flex-col gap-1 px-2" role="navigation" aria-label="Main navigation">
        {navItems.map((item) => {
          const isActive = currentPage === item.id
          return (
            <button
              key={item.id}
              type="button"
              onClick={() => onNavigate(item.id)}
              className={cn(
                "flex items-center justify-center gap-3 rounded-lg px-3 py-2.5 text-sm font-medium transition-colors lg:justify-start",
                isActive
                  ? "bg-accent text-foreground"
                  : "text-muted-foreground hover:bg-accent/50 hover:text-foreground"
              )}
              aria-current={isActive ? "page" : undefined}
            >
              <item.icon className="h-5 w-5 shrink-0" />
              <span className="hidden lg:block">{item.label}</span>
            </button>
          )
        })}
      </nav>

      <div className="flex flex-col gap-1 px-2 pb-2">
        <a
          href="https://github.com"
          target="_blank"
          rel="noopener noreferrer"
          className="flex items-center justify-center gap-3 rounded-lg px-3 py-2.5 text-sm font-medium text-muted-foreground transition-colors hover:bg-accent/50 hover:text-foreground lg:justify-start"
        >
          <Github className="h-5 w-5 shrink-0" />
          <span className="hidden lg:block">GitHub</span>
        </a>
      </div>
    </aside>
  )
}
