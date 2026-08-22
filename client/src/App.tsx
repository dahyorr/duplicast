import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { lazy, Suspense, useEffect, useState } from "react";
import { AppSidebar } from "@/components/app-sidebar";
import { Toaster } from "@/components/ui/sonner";
import { useLiveSocket } from "@/lib/live-socket";
import "./index.css";

const DashboardPage = lazy(() => import("@/components/pages/dashboard-page").then((m) => ({ default: m.DashboardPage })));
const StreamPage = lazy(() => import("@/components/pages/stream-page").then((m) => ({ default: m.StreamPage })));
const RelaysPage = lazy(() => import("@/components/pages/relays-page").then((m) => ({ default: m.RelaysPage })));
const LogsPage = lazy(() => import("@/components/pages/logs-page").then((m) => ({ default: m.LogsPage })));
const SettingsPage = lazy(() => import("@/components/pages/settings-page").then((m) => ({ default: m.SettingsPage })));

// Devtools are dev-only and shouldn't ship in the production bundle at all.
const ReactQueryDevtools = import.meta.env.PROD
  ? () => null
  : lazy(() => import("@tanstack/react-query-devtools").then((m) => ({ default: m.ReactQueryDevtools })));

const queryClient = new QueryClient();

const pages: Record<string, React.ComponentType> = {
  dashboard: DashboardPage,
  stream: StreamPage,
  relays: RelaysPage,
  logs: LogsPage,
  settings: SettingsPage,
};

function AppContent() {
  useLiveSocket();

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
        <Suspense fallback={null}>
          <ActivePage />
        </Suspense>
      </main>
    </div>
  );
}

export default function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <AppContent />
      <Toaster />
      <Suspense fallback={null}>
        <ReactQueryDevtools initialIsOpen={false} />
      </Suspense>
    </QueryClientProvider>
  );
}
