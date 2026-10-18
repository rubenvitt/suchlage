import { skipToken, useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import type { AuthOptions } from '../contract/generated/AuthOptions';
import type { FlightDetail } from '../contract/generated/FlightDetail';
import type { FlightSummary } from '../contract/generated/FlightSummary';
import type { Health } from '../contract/generated/Health';
import type { KioskTokenView } from '../contract/generated/KioskTokenView';
import type { Me } from '../contract/generated/Me';
import type { Mission } from '../contract/generated/Mission';
import type { MissionContents } from '../contract/generated/MissionContents';
import type { MissionCreate } from '../contract/generated/MissionCreate';
import type { MissionUpdate } from '../contract/generated/MissionUpdate';
import type { PhotoShot } from '../contract/generated/PhotoShot';
import type { SectionCreate } from '../contract/generated/SectionCreate';
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

/** Schreibt am Einsatz; danach laden die Einsätze neu (auch nach einem Fehler: der Stand kann sich geändert haben). */
function useMissionWrite<V, R>(send: (v: V) => Promise<R>) {
  const client = useQueryClient();
  return useMutation({
    mutationFn: send,
    onSettled: () => client.invalidateQueries({ queryKey: queryKeys.missions }),
  });
}

/** Einsatz anlegen (vorbereitet). */
export const useCreateMission = () =>
  useMissionWrite((input: MissionCreate) => apiSend<Mission>('POST', '/api/missions', input));
/** Vorbereiteten Einsatz starten; 409, wenn schon einer läuft. */
export const useStartMission = () =>
  useMissionWrite((id: number) => apiSend<Mission>('POST', `/api/missions/${id}/start`));
/** Laufenden Einsatz abschließen; 422 während eines Flugs. */
export const useCloseMission = () =>
  useMissionWrite((id: number) => apiSend<Mission>('POST', `/api/missions/${id}/close`));
export const useUpdateMission = () =>
  useMissionWrite(({ id, ...body }: MissionUpdate & { id: number }) =>
    apiSend<Mission>('PUT', `/api/missions/${id}`, body),
  );
export const useAddSection = () =>
  useMissionWrite(({ id, ...body }: SectionCreate & { id: number }) =>
    apiSend<Mission>('POST', `/api/missions/${id}/sections`, body),
  );
/**
 * In diesem Tab gelöschte Einsätze. Ihre offene Ansicht verlässt der Lösch-Dialog selbst; die
 * Umleitung für verschwundene Einsätze (`MissionHeader`) liefe sonst gegen dessen Navigation, die
 * React als Übergang später anwendet als das neu geladene Einsatzverzeichnis.
 */
export const deletedHere = new Set<number>();

/**
 * Einsatz mit allem, was dazugehört, löschen (nur Verwaltung); 422 laufend, 503 Ablage fehlt.
 * `onDeleted` läuft vor dem Neuladen: Die offene Ansicht des Einsatzes fiele sonst kurz auf den laufenden.
 */
export function useDeleteMission(onDeleted: () => void) {
  const client = useQueryClient();
  return useMutation({
    mutationFn: (id: number) => apiSend<void>('DELETE', `/api/missions/${id}`),
    onSuccess: (_, id) => {
      deletedHere.add(id);
      onDeleted();
      for (const queryKey of [queryKeys.missions, queryKeys.flights, queryKeys.photos])
        void client.invalidateQueries({ queryKey });
    },
  });
}
/** Mengen für die Lösch-Rückfrage. */
export const useMissionContents = (id: number) =>
  useQuery({
    queryKey: [...queryKeys.missions, id, 'contents'],
    queryFn: () => apiGet<MissionContents>(`/api/missions/${id}/contents`),
  });

/**
 * Flug samt Spur, Video und Fotos löschen (nur Verwaltung); 422 laufend, 503 Ablage fehlt.
 * `onDeleted` verlässt das Detail vor dem Neuladen, sonst liefe dessen 404-Umleitung dagegen.
 */
export function useDeleteFlight(onDeleted: () => void) {
  const client = useQueryClient();
  return useMutation({
    mutationFn: (id: number) => apiSend<void>('DELETE', `/api/flights/${id}`),
    onSuccess: () => {
      onDeleted();
      for (const queryKey of [queryKeys.missions, queryKeys.flights, queryKeys.photos])
        void client.invalidateQueries({ queryKey });
    },
  });
}
/** Nur die Aufzeichnung eines Flugs löschen; danach zeigt das Detail „Kein Video“. */
export function useDeleteFlightVideo() {
  const client = useQueryClient();
  return useMutation({
    mutationFn: (id: number) => apiSend<void>('DELETE', `/api/flights/${id}/video`),
    onSuccess: (_, id) => client.invalidateQueries({ queryKey: queryKeys.flight(id) }),
  });
}
/** Eine Auslösung mit allen Linsen löschen. Die Kennung enthält `/` und geht darum kodiert. */
export function useDeleteShot() {
  const client = useQueryClient();
  return useMutation({
    mutationFn: (shotId: string) => apiSend<void>('DELETE', `/api/photos/shots/${encodeURIComponent(shotId)}`),
    onSuccess: () => client.invalidateQueries({ queryKey: queryKeys.photos }),
  });
}
