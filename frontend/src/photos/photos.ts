import type { PhotoLens } from '../contract/generated/PhotoLens';
import type { PhotoShot } from '../contract/generated/PhotoShot';
import type { Collection } from '../map/layers';

type Point = { type: 'Point'; coordinates: [number, number] };

export interface PhotoProps {
  id: string;
  /** Uhrzeit der Auslösung, z. B. „14:30“. */
  time: string;
  selected: boolean;
}

export const LENS_LABEL: Record<PhotoLens, string> = {
  thermal: 'Wärme',
  wide: 'Weit',
  zoom: 'Zoom',
  visible: 'Sichtbar',
  unknown: 'Foto',
};

const clock = (ms: number) => new Date(ms).toLocaleTimeString('de-DE', { hour: '2-digit', minute: '2-digit' });

/**
 * Liste vom Server und Live-Strom zusammen, älteste zuerst. Eine Auslösung aus dem Strom ersetzt
 * die aus der Liste: Sie trägt neue Linsen und das fertige Vorschaubild.
 */
export function mergeShots(listed: PhotoShot[] | undefined, live: Record<string, PhotoShot>): PhotoShot[] {
  const byId = new Map((listed ?? []).map((s) => [s.id, s]));
  for (const s of Object.values(live)) byId.set(s.id, s);
  return [...byId.values()].sort((a, b) => a.taken_at_ms - b.taken_at_ms || a.id.localeCompare(b.id));
}

/** Nur Auslösungen mit Position kommen auf die Karte. */
export function photoCollection(shots: PhotoShot[], selectedId: string | null = null): Collection<Point, PhotoProps> {
  return {
    type: 'FeatureCollection',
    features: shots.flatMap((s) =>
      s.position
        ? [
            {
              type: 'Feature' as const,
              geometry: { type: 'Point' as const, coordinates: [s.position.lon, s.position.lat] as [number, number] },
              properties: { id: s.id, time: clock(s.taken_at_ms), selected: s.id === selectedId },
            },
          ]
        : [],
    ),
  };
}

/** Kopfzeile einer Auslösung: Uhrzeit und Höhe über Start, soweit bekannt. */
export function shotTitle(s: PhotoShot): string {
  const height = s.relative_altitude_m == null ? '' : ` · ${Math.round(s.relative_altitude_m)} m über Start`;
  return `${clock(s.taken_at_ms)}${height}`;
}
