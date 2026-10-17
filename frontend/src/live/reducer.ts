import type { ActiveFlight } from '../contract/generated/ActiveFlight';
import type { AircraftState } from '../contract/generated/AircraftState';
import type { FlightSummary } from '../contract/generated/FlightSummary';
import type { FreshnessThresholds } from '../contract/generated/FreshnessThresholds';
import type { GatewayState } from '../contract/generated/GatewayState';
import type { LiveEvent } from '../contract/generated/LiveEvent';
import type { StreamState } from '../contract/generated/StreamState';
import type { SystemStatus } from '../contract/generated/SystemStatus';
import type { TrackPoint } from '../contract/generated/TrackPoint';

export interface LiveState {
  /** Erst nach dem ersten Snapshot gibt es eine Lage; vorher zeigen die Paneele „laden“. */
  ready: boolean;
  /** Serverzeit minus Browserzeit; Frische wird immer gegen die Serveruhr gerechnet. */
  serverOffsetMs: number;
  thresholds: FreshnessThresholds;
  gateways: Record<string, GatewayState>;
  aircraft: Record<string, AircraftState>;
  /** Laufende Flüge nach Flug-ID. */
  flights: Record<number, ActiveFlight>;
  /** Zuletzt beendeter Flug je Fluggeräte-SN. */
  ended: Record<string, FlightSummary>;
  /** Livebild je Fernsteuerungs-SN. */
  streams: Record<string, StreamState>;
  system: SystemStatus | null;
}

export const initialLiveState: LiveState = {
  ready: false,
  serverOffsetMs: 0,
  thresholds: { delayed_after_ms: 6_000, lost_after_ms: 30_000 },
  gateways: {},
  aircraft: {},
  flights: {},
  ended: {},
  streams: {},
  system: null,
};

/** Wie `situation::track::haversine_m` im Kern. */
function haversineM(a: TrackPoint, b: TrackPoint): number {
  const rad = Math.PI / 180;
  const h =
    Math.sin(((b.lat - a.lat) * rad) / 2) ** 2 +
    Math.cos(a.lat * rad) * Math.cos(b.lat * rad) * Math.sin(((b.lon - a.lon) * rad) / 2) ** 2;
  return 2 * 6_371_008.8 * Math.asin(Math.sqrt(h));
}

const byKey = <T>(items: T[], key: (t: T) => string | number) =>
  Object.fromEntries(items.map((i) => [key(i), i]));

/** Reine Zustandsfunktion: gleiche Referenz, wenn sich nichts ändert, sonst neue Objekte nur entlang der Änderung. */
export function applyEvent(s: LiveState, e: LiveEvent, clientNowMs: number): LiveState {
  switch (e.type) {
    case 'snapshot': {
      const snap = e.snapshot;
      return {
        ready: true,
        serverOffsetMs: snap.server_time_ms - clientNowMs,
        thresholds: snap.freshness,
        gateways: byKey(snap.gateways, (g) => g.sn),
        aircraft: byKey(snap.aircraft, (a) => a.sn),
        flights: byKey(snap.active_flights, (f) => f.flight.id),
        ended: s.ended,
        streams: byKey(snap.streams, (st) => st.gateway_sn),
        system: snap.system,
      };
    }
    case 'gateway':
      return { ...s, gateways: { ...s.gateways, [e.gateway.sn]: e.gateway } };
    case 'aircraft':
      return { ...s, aircraft: { ...s.aircraft, [e.aircraft.sn]: e.aircraft } };
    case 'flight_started':
      return { ...s, flights: { ...s.flights, [e.flight.id]: { flight: e.flight, segments: [] } } };
    case 'flight_ended': {
      const flights = { ...s.flights };
      delete flights[e.flight.id];
      return { ...s, flights, ended: { ...s.ended, [e.flight.aircraft_sn]: e.flight } };
    }
    case 'track_point': {
      const active = s.flights[e.flight_id];
      if (!active) return s;
      const segments = active.segments.slice();
      while (segments.length <= e.segment) segments.push([]);
      const target = segments[e.segment]!;
      const last = target.at(-1);
      // Nach einem Neuverbinden kann ein Punkt doppelt kommen; die Spur läuft nur vorwärts.
      if (last && last.t_ms >= e.point.t_ms) return s;
      segments[e.segment] = [...target, e.point];
      // Strecke und Höhe rechnet der Kern erst beim Schließen fest; bis dahin hier nach seiner Regel:
      // Strecke nur innerhalb eines Segments.
      const { distance_m, max_elevation_m } = active.flight;
      const elevation = e.point.elevation_m;
      const flight = {
        ...active.flight,
        distance_m: distance_m + (last ? haversineM(last, e.point) : 0),
        max_elevation_m:
          elevation == null ? max_elevation_m : Math.max(max_elevation_m ?? elevation, elevation),
      };
      return { ...s, flights: { ...s.flights, [e.flight_id]: { flight, segments } } };
    }
    case 'stream':
      return { ...s, streams: { ...s.streams, [e.stream.gateway_sn]: e.stream } };
    case 'system':
      return { ...s, system: e.system };
    default:
      // Ein lange offener Tab kann einen neueren Server erleben; dessen neue Ereignisse lassen den Stand stehen.
      console.warn('Unbekanntes Live-Ereignis – übersprungen');
      return s;
  }
}
