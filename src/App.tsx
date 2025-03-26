import "./global.css";
import MainPage from "./components/MainPage";
import { AppStateProvider } from "./components/AppState";
import { HeroUIProvider } from "@heroui/system";

function App() {
  return (
    <HeroUIProvider
    >
      <AppStateProvider >
        <main className="dark text-foreground bg-background min-h-screen">
          <MainPage />
        </main>
      </AppStateProvider>
    </HeroUIProvider>
  );
}

export default App;
