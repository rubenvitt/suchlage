import { useQuery } from '@tanstack/react-query';
import type { AuthOptions } from '../contract/generated/AuthOptions';
import type { FlightDetail } from '../contract/generated/FlightDetail';
import type { FlightSummary } from '../contract/generated/FlightSummary';
import type { KioskTokenView } from '../contract/generated/KioskTokenView';
import type { Me } from '../contract/generated/Me';
import type { RemoteControllerView } from '../contract/generated/RemoteControllerView';
import type { SettingsView } from '../contract/generated/SettingsView';
import { ApiError, apiGet } from './client';

export const queryKeys = {
  me: ['me'] as const,
  authOptions: ['auth-options'] as const,
  flights: ['flights'] as const,
  flight: (id: number) => ['flights', id] as const,
  settings: ['settings'] as const,
  remoteControllers: ['remote-controllers'] as const,
  kioskTokens: ['kiosk-tokens'] as const,
};

/** 401 ist kein Fehler, sondern „nicht angemeldet“: `null`. */
export function useMe() {
  return useQuery({
    queryKey: queryKeys.me,
    queryFn: async () => {
      try {
        return await apiGet<Me>('/api/me');
      } catch (e) {
        if (e instanceof ApiError && e.status === 401) return null;
        throw e;
      }
    },
    staleTime: 60_000,
  });
}

export const useAuthOptions = () =>
  useQuery({ queryKey: queryKeys.authOptions, queryFn: () => apiGet<AuthOptions>('/api/auth/options') });
// Kein Polling: der LiveProvider (F2) erneuert die Liste bei `flight_started` und `flight_ended`.
export const useFlights = () =>
  useQuery({ queryKey: queryKeys.flights, queryFn: () => apiGet<FlightSummary[]>('/api/flights') });
export const useFlight = (id: number) =>
  useQuery({ queryKey: queryKeys.flight(id), queryFn: () => apiGet<FlightDetail>(`/api/flights/${id}`) });
export const useSettings = () =>
  useQuery({ queryKey: queryKeys.settings, queryFn: () => apiGet<SettingsView>('/api/settings') });
export const useRemoteControllers = () =>
  useQuery({
    queryKey: queryKeys.remoteControllers,
    queryFn: () => apiGet<RemoteControllerView[]>('/api/remote-controllers'),
  });
export const useKioskTokens = () =>
  useQuery({ queryKey: queryKeys.kioskTokens, queryFn: () => apiGet<KioskTokenView[]>('/api/kiosk-tokens') });
