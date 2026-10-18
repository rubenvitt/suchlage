import { skipToken, useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import type { AuthOptions } from '../contract/generated/AuthOptions';
import type { FlightDetail } from '../contract/generated/FlightDetail';
import type { FlightSummary } from '../contract/generated/FlightSummary';
import type { Health } from '../contract/generated/Health';
import type { KioskTokenView } from '../contract/generated/KioskTokenView';
import type { Me } from '../contract/generated/Me';
import type { Mission } from '../contract/generated/Mission';
import type { MissionCreate } from '../contract/generated/MissionCreate';
import type { PhotoShot } from '../contract/generated/PhotoShot';
import type { RemoteControllerView } from '../contract/generated/RemoteControllerView';
import type { SettingsView } from '../contract/generated/SettingsView';
import { scopeQuery, type MissionScope } from '../missions/scope';
import { ApiError, apiGet, apiSend } from './client';

export const queryKeys = {
  me: ['me'] as const,
  authOptions: ['auth-options'] as const,
  flights: ['flights'] as const,
  flight: (id: number | null) => ['flights', id] as const,
  settings: ['settings'] as const,
  remoteControllers: ['remote-controllers'] as const,
  kioskTokens: ['kiosk-tokens'] as const,
  health: ['health'] as const,
  missions: ['missions'] as const,
  /** Fotos der Lagekarte je Einsatz und je Flug; der Live-Strom ergänzt beide. */
  photos: ['photos'] as const,
  mapPhotos: (scope: string) => ['photos', 'map', scope] as const,
  flightPhotos: (id: number | null) => ['photos', 'flight', id] as const,
};

/** Abstand der Selbstheilung, solange `/api/me` noch nie geantwortet hat. */
export const ME_RETRY_MS = 5_000;

/**
 * 401 ist kein Fehler, sondern „nicht angemeldet“: `null`. Scheitert schon der erste Abruf, fragt
 * die Abfrage alle `ME_RETRY_MS` neu: Das Großbild hat keine Bedienung und muss sich selbst fangen.
 */
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
    refetchInterval: (q) => (q.state.status === 'error' && q.state.data === undefined ? ME_RETRY_MS : false),
  });
}

export const useAuthOptions = () =>
  useQuery({ queryKey: queryKeys.authOptions, queryFn: () => apiGet<AuthOptions>('/api/auth/options') });
// Kein Polling: der LiveProvider (F2) erneuert die Liste bei `flight_started` und `flight_ended`.
/** Ohne `enabled` (Einsätze noch unbekannt) fragt die Abfrage nicht: Der Scope wäre sonst „ohne“. */
export const useFlights = (scope: MissionScope, enabled = true) => {
  const mission = scopeQuery(scope);
  return useQuery({
    queryKey: [...queryKeys.flights, mission],
    queryFn: enabled ? () => apiGet<FlightSummary[]>(`/api/flights?mission=${mission}`) : skipToken,
  });
};
/** `null` steht für eine Adresse ohne gültige Flugnummer: Dann fragt die Abfrage gar nicht erst. */
export const useFlight = (id: number | null) =>
  useQuery({
    queryKey: queryKeys.flight(id),
    queryFn: id == null ? skipToken : () => apiGet<FlightDetail>(`/api/flights/${id}`),
  });
export const useSettings = () =>
  useQuery({ queryKey: queryKeys.settings, queryFn: () => apiGet<SettingsView>('/api/settings') });
export const useRemoteControllers = () =>
  useQuery({
    queryKey: queryKeys.remoteControllers,
    queryFn: () => apiGet<RemoteControllerView[]>('/api/remote-controllers'),
  });
/** Alle Einsätze; der Live-Strom erneuert die Liste bei `missions_changed`. */
export const useMissions = () =>
  useQuery({ queryKey: queryKeys.missions, queryFn: () => apiGet<Mission[]>('/api/missions') });
export const useKioskTokens = () =>
  useQuery({ queryKey: queryKeys.kioskTokens, queryFn: () => apiGet<KioskTokenView[]>('/api/kiosk-tokens') });
/** Laufender Stand des Servers. Ohne `staleTime`: Nach einem Update zeigt die offene Seite beim nächsten Blick den neuen. */
export const useHealth = () =>
  useQuery({ queryKey: queryKeys.health, queryFn: () => apiGet<Health>('/api/health') });
/**
 * Auslösungen der Lagekarte: im Einsatz alle, „ohne Einsatz“ die der letzten 24 h; neue bringt der
 * Live-Strom (`photo_shot`). Ohne `enabled` fragt die Abfrage nicht.
 */
export const useMapPhotos = (scope: MissionScope, enabled = true) => {
  const mission = scopeQuery(scope);
  return useQuery({
    queryKey: queryKeys.mapPhotos(mission),
    queryFn: enabled ? () => apiGet<PhotoShot[]>(`/api/photos?mission=${mission}`) : skipToken,
  });
};
/** Alle Fotos eines Flugs; `null` fragt nicht. */
export const useFlightPhotos = (id: number | null) =>
  useQuery({
    queryKey: queryKeys.flightPhotos(id),
    queryFn: id == null ? skipToken : () => apiGet<PhotoShot[]>(`/api/flights/${id}/photos`),
  });

/**
 * Einsatz anlegen, mit `start` gleich starten. Danach laden die Einsätze neu, auch nach einem
 * Fehler: Ein angelegter Einsatz, dessen Start scheitert, steht dann unter „Vorbereitet“.
 */
export function useCreateMission() {
  const client = useQueryClient();
  return useMutation({
    mutationFn: async ({ input, start }: { input: MissionCreate; start: boolean }) => {
      const created = await apiSend<Mission>('POST', '/api/missions', input);
      return start ? apiSend<Mission>('POST', `/api/missions/${created.id}/start`) : created;
    },
    onSettled: () => client.invalidateQueries({ queryKey: queryKeys.missions }),
  });
}

/** Vorbereiteten Einsatz starten; 409, wenn schon einer läuft. */
export function useStartMission() {
  const client = useQueryClient();
  return useMutation({
    mutationFn: (id: number) => apiSend<Mission>('POST', `/api/missions/${id}/start`),
    onSettled: () => client.invalidateQueries({ queryKey: queryKeys.missions }),
  });
}
