import "./global.css";
import MainPage from "./components/MainPage";
import { AppStateProvider } from "./components/AppState";
import { HeroUIProvider } from "@heroui/system";
import { ToastProvider } from "@heroui/toast";
import Navbar from "./components/Navbar";

function App() {
  return (
    <HeroUIProvider>
      <ToastProvider />
      <AppStateProvider >
        <main className="dark text-foreground bg-background min-h-screen">
          <Navbar />
          <div className="container mx-auto max-w-6xl px-4">
          <MainPage />
          </div>
        </main>
      </AppStateProvider>
    </HeroUIProvider>
  );
}

export default App;
