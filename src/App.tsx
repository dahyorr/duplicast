import "./global.css";
import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { HeroUIProvider } from "@heroui/system";

import { listen } from "@tauri-apps/api/event";
import MainPage from "./components/MainPage";

function App() {
  const [ports, setPorts] = useState({ rtmp_port: 0, file_port: 0 });
  const [serversReady, setServersReady] = useState(false);
  // const [greetMsg, setGreetMsg] = useState("");
  const [name, setName] = useState("");

  // async function greet() {
  //   // Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
  //   setGreetMsg(await invoke("greet", { name }));
  // }

  async function get_ports() {
    setPorts(await invoke("get_ports"));
  }

  async function check_if_ready() {
    const ready = await invoke("check_if_ready");
    if (ready) {
      await get_ports();
      setServersReady(true);
      console.log("RTMP + File Server Ready ✅");
    }
  }

  useEffect(() => {
    const unlisten = listen("servers-ready", (event) => {
      console.log("RTMP + File Server Ready ✅", event.payload);
      setPorts(event.payload as any);
      setServersReady(true);
    });

    return () => {
      unlisten.then((u) => u());
    };
  })


  useEffect(() => {
    // call check if ready every 2 seconds
    if(serversReady) return;
    const interval = setInterval(() => {
      check_if_ready();
    }
    , 2000);
    return () => clearInterval(interval);
  }, [serversReady]);

  console.log(serversReady, ports, "wrwiejuhbt");



  return (
    <HeroUIProvider>
      <MainPage seversReady={serversReady} ports={ports} />
    </HeroUIProvider>
  );
}

export default App;
