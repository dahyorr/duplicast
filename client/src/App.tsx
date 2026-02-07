import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { ReactQueryDevtools } from '@tanstack/react-query-devtools';
import { useEffect, useState } from "react";
import { AppSidebar } from "@/components/app-sidebar";
import { DashboardPage } from "@/components/pages/dashboard-page";
import { StreamPage } from "@/components/pages/stream-page";
import { RelaysPage } from "@/components/pages/relays-page";
import { LogsPage } from "@/components/pages/logs-page";
import { SettingsPage } from "@/components/pages/settings-page";
import { Toaster } from "@/components/ui/sonner";
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
  const [currentPage, setCurrentPage] = useState(() => {
    const params = new URLSearchParams(window.location.search);
    return params.get('page') || 'dashboard';
  });

  // Handle navigation and update URL
  const handleNavigate = (page: string) => {
    const url = new URL(window.location.href);
    url.searchParams.set('page', page);
    window.history.pushState({}, '', url);
    setCurrentPage(page);
  };

  // Listen for popstate (back/forward buttons)
  useEffect(() => {
    const handlePopState = () => {
      const params = new URLSearchParams(window.location.search);
      setCurrentPage(params.get('page') || 'dashboard');
    };
    window.addEventListener('popstate', handlePopState);
    return () => window.removeEventListener('popstate', handlePopState);
  }, []);

  const ActivePage = pages[currentPage] || DashboardPage;

  return (
    <div className="flex h-screen overflow-hidden bg-background text-foreground">
      <AppSidebar currentPage={currentPage} onNavigate={handleNavigate} />
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
      <Toaster />
      <ReactQueryDevtools initialIsOpen={false} />
    </QueryClientProvider>
  );
}
