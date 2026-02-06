import axios from 'axios';
import type { Stream, Relay, Stats, CreateRelayRequest, StartRelayRequest, StreamInfo } from './types';

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

export default api;
