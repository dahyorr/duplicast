import "./global.css";
import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { HeroUIProvider } from "@heroui/system";
import { Button } from "@heroui/button";
import Navbar from "./components/Navbar";
import StreamInputDetails from "./components/StreamInputDetails";
import { Divider } from "@heroui/divider";
import StreamPreview from "./components/StreamPreview";

function App() {
  const [greetMsg, setGreetMsg] = useState("");
  const [name, setName] = useState("");

  async function greet() {
    // Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
    setGreetMsg(await invoke("greet", { name }));
  }

  return (
    <HeroUIProvider>
      <div className="container mx-auto">
        <Navbar />

        <div className="mt-16">
          <p>Stream Input</p>
          <div className="flex">
            <StreamInputDetails />
            <Divider orientation="vertical" />
            <StreamPreview />
          </div>
        </div>
        <p>Stream Destination</p>
        <div>

        </div>
      </div>
    </HeroUIProvider>
  );
}

export default App;
