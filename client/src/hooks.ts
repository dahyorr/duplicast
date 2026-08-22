import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import * as api from './api';
import type { Config, CreateRelayRequest, StartRelayRequest } from './types';

// Streams
export const useStreams = () => {
  return useQuery({
    queryKey: ['streams'],
    queryFn: async () => {
      const { data } = await api.getStreams();
      return data;
    },
    refetchInterval: 2000, // Refresh every 2 seconds
  });
};

export const useStream = (id: string) => {
  return useQuery({
    queryKey: ['streams', id],
    queryFn: async () => {
      const { data } = await api.getStream(id);
      return data;
    },
    refetchInterval: 2000,
  });
};

export const useStreamInfo = (id: string) => {
  return useQuery({
    queryKey: ['streams', id, 'info'],
    queryFn: async () => {
      const { data } = await api.getStreamInfo(id);
      return data;
    },
    refetchInterval: 2000,
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
    refetchInterval: 1000, // Refresh every second for stats
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
    refetchInterval: 2000,
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

// Logs
export const useLogs = () => {
  return useQuery({
    queryKey: ['logs'],
    queryFn: async () => {
      const { data } = await api.getLogs();
      return data;
    },
    refetchInterval: 5000,
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
