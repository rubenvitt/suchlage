import type { GeoPoint } from '../contract/generated/GeoPoint';
import type { MapElement } from '../contract/generated/MapElement';
import type { MapElementKind } from '../contract/generated/MapElementKind';
import type { Collection } from './layers';

type LngLat = [number, number];
type ElementGeometry =
  | { type: 'Point'; coordinates: LngLat }
  | { type: 'LineString'; coordinates: LngLat[] }
  | { type: 'Polygon'; coordinates: LngLat[][] };

export interface ElementProps {
  id: string;
  name: string;
  color: string;
  selected: boolean;
}

export const KIND_LABEL: Record<MapElementKind, string> = {
  point: 'Punkt',
  line: 'Linie',
  polygon: 'Fläche',
};

/** So viele Punkte braucht eine Form mindestens. */
export const MIN_POINTS: Record<MapElementKind, number> = { point: 1, line: 2, polygon: 3 };

const lngLat = (p: GeoPoint): LngLat => [p.lon, p.lat];

/** GeoJSON schließt Ringe; der Server speichert sie offen. */
function geometry(kind: MapElementKind, points: GeoPoint[]): ElementGeometry {
  const coords = points.map(lngLat);
  if (kind === 'point') return { type: 'Point', coordinates: coords[0]! };
  if (kind === 'line') return { type: 'LineString', coordinates: coords };
  return { type: 'Polygon', coordinates: [[...coords, coords[0]!]] };
}

/** Alle Elemente in einer Quelle; die Ebenen filtern nach Geometrietyp. Leere Formen fallen weg. */
export function elementCollection(
  elements: MapElement[],
  selectedId: string | null = null,
): Collection<ElementGeometry, ElementProps> {
  return {
    type: 'FeatureCollection',
    features: elements
      .filter((e) => e.points.length >= MIN_POINTS[e.kind])
      .map((e) => ({
        type: 'Feature',
        geometry: geometry(e.kind, e.points),
        properties: { id: e.id, name: e.name, color: e.color, selected: e.id === selectedId },
      })),
  };
}

/**
 * Je Element ein Beschriftungspunkt: Punkte an ihrer Stelle, Flächen in der Mitte ihrer Ecken.
 * MapLibre beschriftet Flächen sonst je Kachel und damit mehrfach. Linien beschriftet die Linie selbst.
 */
export function elementLabelCollection(
  elements: MapElement[],
): Collection<{ type: 'Point'; coordinates: LngLat }, { id: string; name: string; kind: MapElementKind }> {
  return {
    type: 'FeatureCollection',
    features: elements
      .filter((e) => e.kind !== 'line' && e.points.length >= MIN_POINTS[e.kind])
      .map((e) => {
        const lat = e.points.reduce((sum, p) => sum + p.lat, 0) / e.points.length;
        const lon = e.points.reduce((sum, p) => sum + p.lon, 0) / e.points.length;
        return {
          type: 'Feature',
          geometry: { type: 'Point', coordinates: [lon, lat] },
          properties: { id: e.id, name: e.name, kind: e.kind },
        };
      }),
  };
}

/** Eine Form im Entstehen. */
export interface Draft {
  kind: MapElementKind;
  points: GeoPoint[];
}

export const startDraft = (kind: MapElementKind): Draft => ({ kind, points: [] });

/** Ein Punkt hat nur eine Koordinate: Ein weiterer Klick ersetzt sie. */
export function addVertex(d: Draft, p: GeoPoint): Draft {
  return { ...d, points: d.kind === 'point' ? [p] : [...d.points, p] };
}

export const removeLastVertex = (d: Draft): Draft => ({ ...d, points: d.points.slice(0, -1) });

export const canFinish = (d: Draft): boolean => d.points.length >= MIN_POINTS[d.kind];

/** Vorschau: die bisherigen Eckpunkte und die Form, sobald es eine Linie gibt. */
export function draftCollection(d: Draft | null): Collection<ElementGeometry, { vertex: boolean }> {
  if (!d) return { type: 'FeatureCollection', features: [] };
  const vertices = d.points.map((p) => ({
    type: 'Feature' as const,
    geometry: { type: 'Point' as const, coordinates: lngLat(p) },
    properties: { vertex: true },
  }));
  const shape =
    d.kind !== 'point' && d.points.length >= 2
      ? [
          {
            type: 'Feature' as const,
            geometry: geometry(d.points.length >= 3 ? d.kind : 'line', d.points),
            properties: { vertex: false },
          },
        ]
      : [];
  return { type: 'FeatureCollection', features: [...shape, ...vertices] };
}

/** „Fläche 3“: die nächste freie Nummer je Form. */
export function defaultName(kind: MapElementKind, elements: MapElement[]): string {
  const base = KIND_LABEL[kind];
  const pattern = new RegExp(`^${base} (\\d+)$`);
  const used = elements.map((e) => Number(pattern.exec(e.name)?.[1] ?? 0));
  return `${base} ${Math.max(0, ...used) + 1}`;
}

/** Hinweis in der Zeichenleiste. */
export function draftHint(d: Draft): string {
  const n = d.points.length;
  if (d.kind === 'point') return 'In die Karte klicken, um den Punkt zu setzen.';
  const noun = d.kind === 'line' ? (n === 1 ? 'Punkt' : 'Punkte') : n === 1 ? 'Eckpunkt' : 'Eckpunkte';
  const need = MIN_POINTS[d.kind] - n;
  return need > 0
    ? `${n} ${noun} · noch mindestens ${need} setzen`
    : `${n} ${noun} · „Fertig“ oder Eingabetaste schließt ab`;
}
