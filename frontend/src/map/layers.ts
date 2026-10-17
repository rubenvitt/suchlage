import type { AircraftState } from '../contract/generated/AircraftState';
import type { FreshnessThresholds } from '../contract/generated/FreshnessThresholds';
import type { GatewayState } from '../contract/generated/GatewayState';
import type { GeoPoint } from '../contract/generated/GeoPoint';
import type { TrackPoint } from '../contract/generated/TrackPoint';
import { ageLabel, freshnessOf, type Freshness } from '../live/freshness';

type LngLat = [number, number];
type Point = { type: 'Point'; coordinates: LngLat };
type Line = { type: 'LineString'; coordinates: LngLat[] };
export type Bounds = [LngLat, LngLat];

export interface Collection<G, P> {
  type: 'FeatureCollection';
  features: { type: 'Feature'; geometry: G; properties: P }[];
}

const collection = <G, P>(features: { geometry: G; properties: P }[]): Collection<G, P> => ({
  type: 'FeatureCollection',
  features: features.map((f) => ({ type: 'Feature', ...f })),
});

const point = (p: GeoPoint): Point => ({ type: 'Point', coordinates: [p.lon, p.lat] });

/** Frische und Alter eines Live-Werts; frische Werte tragen kein Alter, die Karte schreibt dann nichts. */
function aged(lastSeenMs: number, serverNow: number, t: FreshnessThresholds) {
  const freshness = freshnessOf(lastSeenMs, serverNow, t);
  return { freshness, age: freshness === 'fresh' ? '' : ageLabel(serverNow - lastSeenMs) };
}

/** Je Flug die Segmente als eigene Linien: eine Telemetrielücke bleibt auf der Karte eine Lücke. */
export function trackCollection(segmentsList: TrackPoint[][][]): Collection<Line, { segment: number }> {
  return collection(
    segmentsList.flatMap((segments) =>
      segments
        .filter((s) => s.length > 1)
        .map((s, i) => ({
          geometry: { type: 'LineString' as const, coordinates: s.map((p): LngLat => [p.lon, p.lat]) },
          properties: { segment: i },
        })),
    ),
  );
}

export function aircraftCollection(aircraft: AircraftState[], serverNow: number, t: FreshnessThresholds) {
  return collection<Point, { sn: string; heading: number; freshness: Freshness; age: string }>(
    aircraft.flatMap((a) => {
      const p = a.telemetry.position;
      if (!p) return [];
      const heading = a.telemetry.heading_deg ?? 0;
      return [
        { geometry: point(p), properties: { sn: a.sn, heading, ...aged(a.last_seen_ms, serverNow, t) } },
      ];
    }),
  );
}

export function gatewayCollection(gateways: GatewayState[], serverNow: number, t: FreshnessThresholds) {
  return collection<Point, { sn: string; label: string; freshness: Freshness; age: string }>(
    gateways.flatMap((g) => {
      if (!g.position) return [];
      const label = g.label ?? g.sn;
      return [
        {
          geometry: point(g.position),
          properties: { sn: g.sn, label, ...aged(g.last_seen_ms, serverNow, t) },
        },
      ];
    }),
  );
}

export function homeCollection(aircraft: AircraftState[]) {
  return collection<Point, { sn: string }>(
    aircraft.flatMap((a) =>
      a.telemetry.home ? [{ geometry: point(a.telemetry.home), properties: { sn: a.sn } }] : [],
    ),
  );
}

/** Umriss aller Punkte (Südwest, Nordost); `null`, wenn es keinen gibt. */
export function boundsOf(groups: GeoPoint[][]): Bounds | null {
  const pts = groups.flat();
  if (pts.length === 0) return null;
  const lons = pts.map((p) => p.lon);
  const lats = pts.map((p) => p.lat);
  return [
    [Math.min(...lons), Math.min(...lats)],
    [Math.max(...lons), Math.max(...lats)],
  ];
}
