import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { useState } from "react";
import { AppSidebar } from "@/components/app-sidebar";
import { DashboardPage } from "@/components/pages/dashboard-page";
import { StreamPage } from "@/components/pages/stream-page";
import { RelaysPage } from "@/components/pages/relays-page";
import { LogsPage } from "@/components/pages/logs-page";
import { SettingsPage } from "@/components/pages/settings-page";
import "./index.css";

const queryClient = new QueryClient();

const pages: Record<string, React.ComponentType> = {
  dashboard: DashboardPage,
  stream: StreamPage,
  relays: RelaysPage,
  logs: LogsPage,
  settings: SettingsPage,
};

function AppContent() {
  const [currentPage, setCurrentPage] = useState("dashboard");
  const ActivePage = pages[currentPage] || DashboardPage;

  return (
    <div className="flex h-screen overflow-hidden bg-background text-foreground">
      <AppSidebar currentPage={currentPage} onNavigate={setCurrentPage} />
      <main className="flex-1 overflow-y-auto p-6 lg:p-8">
        <ActivePage />
      </main>
    </div>
  );
}

export default function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <AppContent />
    </QueryClientProvider>
  );
}
