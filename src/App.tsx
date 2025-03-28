import "./global.css";
import MainPage from "./components/MainPage";
import { AppStateProvider } from "./components/AppState";
import { HeroUIProvider } from "@heroui/system";
import { ToastProvider } from "@heroui/toast";

function App() {
  return (
    <HeroUIProvider>
      <ToastProvider/>
      <AppStateProvider >
        <main className="dark text-foreground bg-background min-h-screen">
          <MainPage />
        </main>
      </AppStateProvider>
    </HeroUIProvider>
  );
}

export default App;
