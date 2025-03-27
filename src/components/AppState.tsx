import { listen } from "@tauri-apps/api/event";

import { invoke } from "@tauri-apps/api/core";

import { createContext, PropsWithChildren, useEffect, useState } from "react";

interface AppState {
  serversReady: boolean;
  sourceActive: boolean;
  ports: { rtmp_port: number, file_port: number }
}

const AppContext = createContext<AppState>({
  serversReady: false,
  sourceActive: false,
  ports: { rtmp_port: 0, file_port: 0 }
});

const AppStateProvider = ({ children }: PropsWithChildren) => {
  const [ports, setPorts] = useState({ rtmp_port: 0, file_port: 0 });
  const [serversReady, setServersReady] = useState(false);
  const [sourceActive, setSourceActive] = useState(false);

  async function get_ports() {
    setPorts(await invoke("get_ports"));
  }

  async function check_if_ready() {
    const ready = await invoke("check_if_ready");
    if (ready) {
      await get_ports();
      setServersReady(true);
      console.log("RTMP + File Server Ready ✅");
      const sourceActive = await invoke("check_if_stream_active") as boolean;
      setSourceActive(sourceActive);
    }
  }

  useEffect(() => {
    const unlistenPromise = listen("servers-ready", (event) => {
      console.log("RTMP + File Server Ready ✅", event.payload);
      setPorts(event.payload as any);
      setServersReady(true);
    });

    return () => {
      unlistenPromise.then((u) => u());
    };
  }, []);

  useEffect(() => {
    const unlistenStreamActive = listen('stream-active', ({ payload }) => {
      console.log('Stream started:', payload)
      // restart player
      setSourceActive(true)
    })
    const unlistenStreamEnded = listen('stream-ended', ({ payload }) => {
      console.log('Stream ended:', payload)
      setSourceActive(false)
    })
    return () => {

      unlistenStreamActive.then((u) => u());
      unlistenStreamEnded.then((u) => u());
    }
  }, [])


  useEffect(() => {
    // call check if ready every 2 seconds
    if (serversReady) return;
    const interval = setInterval(() => {
      check_if_ready();
    }
      , 2000);
    return () => clearInterval(interval);
  }, [serversReady]);


  const value = {
    serversReady,
    sourceActive,
    ports
  } as AppState;

  return (
    <AppContext.Provider value={value}>
      {children}
    </AppContext.Provider>
  )
}

export { AppStateProvider, AppContext }