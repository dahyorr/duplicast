import "./global.css";
import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { HeroUIProvider } from "@heroui/system";
import { Button } from "@heroui/button";

function App() {
  const [greetMsg, setGreetMsg] = useState("");
  const [name, setName] = useState("");

  async function greet() {
    // Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
    setGreetMsg(await invoke("greet", { name }));
  }

  return (
    <HeroUIProvider>
      <h1 className="text-3xl font-bold underline">
        Hello world!
      </h1>
      <main className="container">
        <Button>TestButton</Button>
      </main>
    </HeroUIProvider>
  );
}

export default App;
