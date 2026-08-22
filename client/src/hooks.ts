import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import * as api from './api';
import type { Config, CreateRelayRequest, StartRelayRequest } from './types';

// Streams, stats, and relays are kept fresh in real time by the WebSocket
// connection (see lib/live-socket.ts), which pushes straight into these same
// query keys. The queryFn here only matters for the very first load (before the
// socket connects) and the refetchInterval is just a fallback safety net in case
// the socket is ever down for an extended period - not the primary update path.
const LIVE_DATA_FALLBACK_INTERVAL = 30_000;

// Streams
export const useStreams = () => {
  return useQuery({
    queryKey: ['streams'],
    queryFn: async () => {
      const { data } = await api.getStreams();
      return data;
    },
    refetchInterval: LIVE_DATA_FALLBACK_INTERVAL,
  });
};

export const useStream = (id: string) => {
  return useQuery({
    queryKey: ['streams', id],
    queryFn: async () => {
      const { data } = await api.getStream(id);
      return data;
    },
    refetchInterval: LIVE_DATA_FALLBACK_INTERVAL,
  });
};

export const useStreamInfo = (id: string) => {
  return useQuery({
    queryKey: ['streams', id, 'info'],
    queryFn: async () => {
      const { data } = await api.getStreamInfo(id);
      return data;
    },
    refetchInterval: LIVE_DATA_FALLBACK_INTERVAL,
  });
};

// Stats
export const useStats = () => {
  return useQuery({
    queryKey: ['stats'],
    queryFn: async () => {
      const { data } = await api.getStats();
      return data;
    },
    refetchInterval: LIVE_DATA_FALLBACK_INTERVAL,
  });
};

// Relays
export const useRelays = () => {
  return useQuery({
    queryKey: ['relays'],
    queryFn: async () => {
      const { data } = await api.getRelays();
      return data;
    },
    refetchInterval: LIVE_DATA_FALLBACK_INTERVAL,
  });
};

export const useCreateRelay = () => {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (data: CreateRelayRequest) => api.createRelay(data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['relays'] });
    },
  });
};

export const useDeleteRelay = () => {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (id: string) => api.deleteRelay(id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['relays'] });
    },
  });
};

export const useStartRelay = () => {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: ({ id, data }: { id: string; data: StartRelayRequest }) =>
      api.startRelay(id, data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['relays'] });
      queryClient.invalidateQueries({ queryKey: ['streams'] });
    },
  });
};

export const useStopRelay = () => {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (id: string) => api.stopRelay(id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['relays'] });
    },
  });
};

// Logs - real-time pushed over the WebSocket as they're created (see
// lib/live-socket.ts); the fallback interval here only matters if the socket
// is down.
export const useLogs = () => {
  return useQuery({
    queryKey: ['logs'],
    queryFn: async () => {
      const { data } = await api.getLogs();
      return data;
    },
    refetchInterval: LIVE_DATA_FALLBACK_INTERVAL,
  });
};

// Config
export const useConfig = () => {
  return useQuery({
    queryKey: ['config'],
    queryFn: async () => {
      const { data } = await api.getConfig();
      return data;
    },
  });
};

export const useSaveConfig = () => {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (config: Config) => api.saveConfig(config),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['config'] });
    },
  });
};
