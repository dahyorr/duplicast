// flv.js ships no TypeScript types and there's no @types/flv.js package.
// Minimal ambient declaration covering only the API this project actually uses.
declare module "flv.js" {
  export interface FlvPlayer {
    attachMediaElement(el: HTMLMediaElement): void;
    detachMediaElement(): void;
    load(): void;
    unload(): void;
    play(): Promise<void>;
    pause(): void;
    destroy(): void;
    on(event: string, handler: (...args: unknown[]) => void): void;
  }

  export interface MediaDataSource {
    type: string;
    url: string;
    isLive?: boolean;
  }

  export const Events: {
    ERROR: string;
    LOADING_COMPLETE: string;
  };

  const flvjs: {
    isSupported(): boolean;
    createPlayer(source: MediaDataSource): FlvPlayer;
    Events: typeof Events;
  };

  export default flvjs;
}
