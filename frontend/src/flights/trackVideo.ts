import type { GeoPoint } from '../contract/generated/GeoPoint';
import type { TrackPoint } from '../contract/generated/TrackPoint';

/**
 * Zeitpunkt des Spurpunkts, der dem Klick am nächsten liegt; `null` ohne Spur. Auf den kurzen
 * Strecken eines Flugs reicht die ebene Näherung, Längengrade schrumpfen dabei mit der Breite.
 */
export function nearestTrackTime(segments: TrackPoint[][], p: GeoPoint): number | null {
  const shrink = Math.cos((p.lat * Math.PI) / 180);
  let best: TrackPoint | null = null;
  let bestDistance = Infinity;
  for (const point of segments.flat()) {
    const d = (point.lat - p.lat) ** 2 + ((point.lon - p.lon) * shrink) ** 2;
    if (d < bestDistance) {
      best = point;
      bestDistance = d;
    }
  }
  return best?.t_ms ?? null;
}

/** Stelle im Video (s) zu einem Zeitpunkt der Spur; vor dem Livebild der Anfang des Videos. */
export const videoSecond = (tMs: number, videoStartMs: number) => Math.max(0, (tMs - videoStartMs) / 1000);
