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
  IceCandidate,
  LogEntry,
  Config,
} from './types';

const API_BASE_URL = import.meta.env.VITE_API_URL || 'http://localhost:8080';

const api = axios.create({
  baseURL: API_BASE_URL,
  headers: {
    'Content-Type': 'application/json',
  },
});

// Health & Stats
export const getHealth = () => api.get('/api/health');
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
export const sendIceCandidate = (streamId: string, candidate: IceCandidate) =>
  api.post(`/api/streams/${streamId}/webrtc/ice`, candidate);

// Logs
export const getLogs = () => api.get<LogEntry[]>('/api/logs');

// Config
export const getConfig = () => api.get<Config>('/api/config');
export const saveConfig = (config: Config) => api.put<Config>('/api/config', config);

export default api;
