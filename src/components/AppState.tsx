import { listen } from "@tauri-apps/api/event";

import { invoke } from "@tauri-apps/api/core";

import { createContext, PropsWithChildren, useEffect, useState } from "react";
import { AppStateEvents, RelayTarget } from "../typings";

interface AppState {
  serversReady: boolean;
  sourceActive: boolean;
  ports: { rtmp_port: number, file_port: number }
  relayTargets: Record<string, RelayTarget>;
  getRelayTargets: () => Promise<void>;
  resetRelayFailedState: (target: RelayTarget) => void;
}

const AppContext = createContext<AppState>({
  serversReady: false,
  sourceActive: false,
  ports: { rtmp_port: 0, file_port: 0 },
  relayTargets: {},
  getRelayTargets: async () => { },
  resetRelayFailedState: () => { },
});

const AppStateProvider = ({ children }: PropsWithChildren) => {
  const [ports, setPorts] = useState({ rtmp_port: 0, file_port: 0 });
  const [serversReady, setServersReady] = useState(false);
  const [sourceActive, setSourceActive] = useState(false);
  const [relayTargets, setRelayTargets] = useState<Record<string, RelayTarget>>({});

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

  async function getRelayTargets() {
    const targets = await invoke("get_relay_targets") as RelayTarget[];
    const targetMap = targets.reduce((acc, target) => {
      acc[target.id] = target;
      return acc;
    }, {} as Record<string, RelayTarget>);
    setRelayTargets(targetMap);
  }

  const init = async () => {
    await getRelayTargets();
  }

  useEffect(() => {
    if (!serversReady) return;
    init();
  }, [serversReady])

  useEffect(() => {
    const unlistenPromise = listen(AppStateEvents.ServersReady, (event) => {
      console.log("RTMP + File Server Ready ✅", event.payload);
      setPorts(event.payload as any);
      setServersReady(true);
    });

    return () => {
      unlistenPromise.then((u) => u());
    };
  }, []);

  useEffect(() => {
    const unlistenStreamPreviewActive = listen(AppStateEvents.StreamPreviewActive, ({ payload }) => {
      console.log('Stream started:', payload)
      // restart player
      setSourceActive(true)
    })
    const unlistenStreamEnded = listen(AppStateEvents.StreamPreviewEnded, ({ payload }) => {
      console.log('Stream ended:', payload)
      setSourceActive(false)
    })
    return () => {
      unlistenStreamPreviewActive.then((u) => u());
      unlistenStreamEnded.then((u) => u());
    }
  }, [])

  useEffect(() => {
    // relay activity listeners
    if (Object.values(relayTargets).length < 1) return;
    const unlistenRelayActive = listen(AppStateEvents.RelayActive, ({ payload: id }) => {
      console.log('Relay started:', id)
      setRelayTargets(prev => {
        const target = prev[parseInt(id as string)];
        if (target) {
          return { ...prev, [target.id]: { ...target, active: true } }
        }
        return prev;

      })
    })

    const unlistenRelayEnded = listen(AppStateEvents.RelayEnded, ({ payload: id }) => {
      console.log('Relay ended:', id)
      setRelayTargets(prev => {
        const target = prev[parseInt(id as string)];
        if (target) {
          return { ...prev, [target.id]: { ...target, active: false } }
        }
        return prev;
      })
    })

    const unlistenRelayFailed = listen(AppStateEvents.RelayFailed, ({ payload }) => {
      const [id, errorMessage] = payload as [string, string]
      console.log('Relay failed:', id)
      setRelayTargets(prev => {
        const target = prev[parseInt(id)];
        if (target) {
          return { ...prev, [target.id]: { ...target, failed: true, active: false, errorMessage } }
        }
        return prev;
      })
    })

    return () => {
      unlistenRelayActive.then((u) => u());
      unlistenRelayEnded.then((u) => u());
      unlistenRelayFailed.then((u) => u());
    }
  }, [relayTargets])


  useEffect(() => {
    // call check if ready every 2 seconds
    if (serversReady) return;
    const interval = setInterval(() => {
      check_if_ready();
    }
      , 2000);
    return () => clearInterval(interval);
  }, [serversReady]);

  const resetRelayFailedState = (target: RelayTarget) => {
    const updatedTarget = { ...target, failed: false, errorMessage: undefined }
    setRelayTargets(prev => {
      return { ...prev, [target.id]: updatedTarget }
    })
  }


  const value = {
    serversReady,
    sourceActive,
    ports,
    relayTargets,
    getRelayTargets,
    resetRelayFailedState
  } as AppState;

  return (
    <AppContext.Provider value={value}>
      {children}
    </AppContext.Provider>
  )
}

export { AppStateProvider, AppContext }