import type { ActiveFlight } from '../contract/generated/ActiveFlight';
import type { AircraftState } from '../contract/generated/AircraftState';
import type { FlightSummary } from '../contract/generated/FlightSummary';
import type { GatewayState } from '../contract/generated/GatewayState';
import type { StreamState } from '../contract/generated/StreamState';
import type { LiveState } from '../live/reducer';

/** Was eine Ansicht zeigt: eine Fernsteuerung und alles, was an ihr hängt. */
export interface Focus {
  gateway?: GatewayState;
  aircraft?: AircraftState;
  flight?: ActiveFlight;
  ended?: FlightSummary;
  stream?: StreamState;
}

/** Name aus der Verwaltung, sonst die Seriennummer. */
export const gatewayName = (g: GatewayState) => g.label ?? g.sn;

/**
 * Ohne (gültige) Wahl gilt die zuletzt gesehene Fernsteuerung mit Fluggerät, sonst die zuletzt
 * gesehene überhaupt. Baut ein neues Objekt: nie direkt als Selektor in `useLive` verwenden.
 */
export function focusOf(s: LiveState, gatewaySn?: string): Focus {
  const latestFirst = Object.values(s.gateways).sort((a, b) => b.last_seen_ms - a.last_seen_ms);
  const gateway =
    (gatewaySn ? s.gateways[gatewaySn] : undefined) ??
    latestFirst.find((g) => g.aircraft_sn) ??
    latestFirst[0];
  if (!gateway) return {};
  const aircraft = gateway.aircraft_sn ? s.aircraft[gateway.aircraft_sn] : undefined;
  return {
    gateway,
    aircraft,
    flight: aircraft?.flight_id != null ? s.flights[aircraft.flight_id] : undefined,
    ended: aircraft ? s.ended[aircraft.sn] : undefined,
    stream: s.streams[gateway.sn],
  };
}
