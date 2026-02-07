import type { StreamStatus, RelayStatus } from "@/types";

export function getStreamStatus(status: StreamStatus): string {
  return status.toLowerCase();
}

export function getRelayStatus(status: RelayStatus): string {
  return status.toLowerCase();
}
