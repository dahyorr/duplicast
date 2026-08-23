import axios from 'axios';
import type {
  Stream,
  Relay,
  Stats,
  CreateRelayRequest,
  StartRelayRequest,
  StreamInfo,
  WebRTCOffer,
  WebRTCAnswer,
  LogEntry,
  Config,
} from './types';

const API_BASE_URL = import.meta.env.VITE_API_URL || 'http://localhost:8080';
// Must match the server's DUPLICAST_API_TOKEN env var. Only needed if that's set;
// harmless to leave unset for local dev.
const API_TOKEN = import.meta.env.VITE_API_TOKEN as string | undefined;

const api = axios.create({
  baseURL: API_BASE_URL,
  headers: {
    'Content-Type': 'application/json',
    ...(API_TOKEN ? { Authorization: `Bearer ${API_TOKEN}` } : {}),
  },
});

// Stats
export const getStats = () => api.get<Stats>('/api/stats');

// Streams
export const getStreams = () => api.get<Stream[]>('/api/streams');
export const getStream = (id: string) => api.get<Stream>(`/api/streams/${id}`);
export const getStreamInfo = (id: string) => api.get<StreamInfo>(`/api/streams/${id}/info`);

// Relays
export const getRelays = () => api.get<Relay[]>('/api/relays');
export const getRelay = (id: string) => api.get<Relay>(`/api/relays/${id}`);
export const createRelay = (data: CreateRelayRequest) => api.post<Relay>('/api/relays', data);
export const deleteRelay = (id: string) => api.delete(`/api/relays/${id}`);
export const startRelay = (id: string, data: StartRelayRequest) =>
  api.post<Relay>(`/api/relays/${id}/start`, data);
export const stopRelay = (id: string) => api.post<Relay>(`/api/relays/${id}/stop`);

// WebRTC
export const sendWebRTCOffer = (streamId: string, offer: WebRTCOffer) =>
  api.post<WebRTCAnswer>(`/api/streams/${streamId}/webrtc/offer`, offer);
export const sendWebRTCHangup = (streamId: string, sessionId: string) =>
  api.delete(`/api/streams/${streamId}/webrtc/${sessionId}`);

// HTTP-FLV preview (default preview mode - see StreamPreview.tsx)
export const getFlvUrl = (streamId: string) => `${API_BASE_URL}/api/streams/${streamId}/flv`;

// Logs
export const getLogs = () => api.get<LogEntry[]>('/api/logs');

// Config
export const getConfig = () => api.get<Config>('/api/config');
export const saveConfig = (config: Config) => api.put<Config>('/api/config', config);

export default api;
