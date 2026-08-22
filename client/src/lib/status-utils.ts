import type { StreamStatus, RelayStatus } from "@/types";

export function getStreamStatus(status: StreamStatus): string {
  return status.status;
}

export function getRelayStatus(status: RelayStatus): string {
  return status.status;
}
