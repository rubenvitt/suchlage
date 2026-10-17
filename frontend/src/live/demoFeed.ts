import { mapElements, snapshotFlying } from '../contract/fixtures.generated';
import type { LiveConnectionOptions } from './connection';

/** Funkstille je Runde, in Sekunden ab Start – zeigt Schraffur, grauen Marker und neues Segment. */
export const DEMO_GAP = { from_s: 20, to_s: 35 };
const ROUND_S = 60;
/** Ein Kreis von rund 340 m Durchmesser in gut zwei Minuten, also etwa 8,5 m/s wie in den Beispieldaten. */
const MS_PER_RADIAN = 20_000;
const RADIUS_LAT_DEG = 0.0015;
const RADIUS_LON_DEG = 0.0025;

const shiftTime = <T extends { t_ms: number }>(p: T, by: number): T => ({ ...p, t_ms: p.t_ms + by });

/**
 * Vorschau-Modus (`mise run ui`): spielt den Flug aus den Beispieldaten ab, auf die aktuelle Uhr
 * verschoben, danach jede Sekunde Position und Spurpunkt. Gleiche Signatur wie `connectLive`.
 */
export function demoFeed({ onEvent, onStatus }: LiveConnectionOptions): () => void {
  const t0 = Date.now();
  const base = snapshotFlying;
  const shift = t0 - base.server_time_ms;
  const plane = base.aircraft[0]!;
  const flight = base.active_flights[0]!;
  const start = flight.segments.at(-1)!.at(-1)!;
  onEvent({
    type: 'snapshot',
    snapshot: {
      ...base,
      server_time_ms: t0,
      gateways: base.gateways.map((g) => ({ ...g, last_seen_ms: t0 })),
      aircraft: [{ ...plane, last_seen_ms: t0 }],
      active_flights: [
        {
          flight: { ...flight.flight, started_at_ms: flight.flight.started_at_ms + shift },
          segments: flight.segments.map((seg) => seg.map((p) => shiftTime(p, shift))),
        },
      ],
      streams: base.streams.map((st) => ({ ...st, updated_at_ms: st.updated_at_ms + shift })),
    },
  });
  // Wie der Server: direkt nach dem Snapshot alle Kartenelemente.
  onEvent({ type: 'map_elements', elements: mapElements });
  onStatus('open');

  let segment = flight.segments.length - 1;
  let silent = false;
  const id = window.setInterval(() => {
    const now = Date.now();
    const s = Math.round((now - t0) / 1000) % ROUND_S;
    if (s >= DEMO_GAP.from_s && s < DEMO_GAP.to_s) {
      silent = true;
      return;
    }
    if (silent) {
      silent = false;
      segment += 1;
    }
    // Kreis, der am letzten Spurpunkt nach Norden beginnt und rechts herum läuft; Kurs = Winkel.
    const a = (now - t0) / MS_PER_RADIAN;
    const lat = start.lat + RADIUS_LAT_DEG * Math.sin(a);
    const lon = start.lon + RADIUS_LON_DEG * (1 - Math.cos(a));
    const heading = (((a * 180) / Math.PI) % 360 + 360) % 360;
    for (const g of base.gateways) onEvent({ type: 'gateway', gateway: { ...g, last_seen_ms: now } });
    onEvent({
      type: 'aircraft',
      aircraft: { ...plane, last_seen_ms: now, telemetry: { ...plane.telemetry, position: { lat, lon }, heading_deg: heading } },
    });
    onEvent({
      type: 'track_point',
      flight_id: flight.flight.id,
      segment,
      point: { t_ms: now, lat, lon, elevation_m: plane.telemetry.elevation_takeoff_m },
    });
  }, 1_000);
  return () => window.clearInterval(id);
}
