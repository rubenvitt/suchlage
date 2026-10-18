import { liveEvents, snapshotEmpty, snapshotFlying } from '../contract/fixtures.generated';
import type { LiveEvent } from '../contract/generated/LiveEvent';
import { applyEvent, initialLiveState } from './reducer';

const ev = <T extends LiveEvent['type']>(type: T) =>
  liveEvents.find((e) => e.type === type) as Extract<LiveEvent, { type: T }>;
const flying = () =>
  applyEvent(initialLiveState, { type: 'snapshot', snapshot: snapshotFlying }, snapshotFlying.server_time_ms - 500);

test('Snapshot ersetzt den Stand und misst den Uhrversatz', () => {
  const s = flying();
  expect(s.ready).toBe(true);
  expect(s.serverOffsetMs).toBe(500);
  expect(Object.keys(s.aircraft)).toEqual(['ACSIM0001']);
  expect(s.flights[7]!.segments).toHaveLength(2);
  const empty = applyEvent(s, { type: 'snapshot', snapshot: snapshotEmpty }, 0);
  expect(empty.aircraft).toEqual({});
});

test('Spurpunkt hängt an das richtige Segment, Duplikate fallen weg', () => {
  const point = ev('track_point');
  const before = flying();
  const once = applyEvent(before, point, 0);
  expect(once.flights[7]!.segments[1]!.at(-1)).toEqual(point.point);
  expect(before.flights[7]!.segments[1]).toHaveLength(2);
  const twice = applyEvent(once, point, 0);
  expect(twice).toBe(once);
  const newSegment = applyEvent(
    once,
    { ...point, segment: 2, point: { ...point.point, t_ms: point.point.t_ms + 20_000 } },
    0,
  );
  expect(newSegment.flights[7]!.segments).toHaveLength(3);
  expect(newSegment.flights[7]!.segments[0]).toBe(once.flights[7]!.segments[0]);
});

test('Spurpunkte führen Strecke und max. Höhe des laufenden Flugs mit, nicht über die Lücke', () => {
  let s = applyEvent(initialLiveState, { type: 'snapshot', snapshot: snapshotEmpty }, 0);
  const started = ev('flight_started');
  // So meldet der Kern einen frisch begonnenen Flug.
  s = applyEvent(s, { ...started, flight: { ...started.flight, distance_m: 0, max_elevation_m: null } }, 0);
  const id = started.flight.id;
  const at = (segment: number, t_ms: number, lat: number, elevation_m: number | null): LiveEvent => ({
    type: 'track_point',
    flight_id: id,
    segment,
    point: { t_ms, lat, lon: 10, elevation_m },
  });
  s = applyEvent(s, at(0, 0, 52.0, 5), 0);
  s = applyEvent(s, at(0, 2_000, 52.001, 30), 0);
  s = applyEvent(s, at(1, 99_000, 52.1, null), 0);
  const flight = s.flights[id]!.flight;
  expect(flight.distance_m).toBeCloseTo(111.2, 0);
  expect(flight.max_elevation_m).toBe(30);
});

test('Spurpunkt eines unbekannten Flugs ändert nichts', () => {
  const s = flying();
  expect(applyEvent(s, { ...ev('track_point'), flight_id: 99 }, 0)).toBe(s);
});

test('Flugende entfernt den aktiven Flug und merkt ihn je Fluggerät', () => {
  const s = applyEvent(flying(), ev('flight_ended'), 0);
  expect(s.flights[7]).toBeUndefined();
  expect(s.ended['ACSIM0001']!.end_reason).toBe('landed');
});

test('Flugstart legt einen leeren Flug an, Stream und System werden übernommen', () => {
  let s = applyEvent(initialLiveState, { type: 'snapshot', snapshot: snapshotEmpty }, 0);
  s = applyEvent(s, ev('flight_started'), 0);
  expect(s.flights[7]!.segments).toEqual([]);
  s = applyEvent(s, ev('stream'), 0);
  expect(s.streams['RCSIM0001']!.phase).toBe('failed');
  s = applyEvent(s, ev('system'), 0);
  expect(s.system!.unknown_fields[0]!.key).toBe('aircraft.wind_speed_extra');
});

test('Unbekanntes Ereignis eines neueren Servers lässt den Stand unverändert', () => {
  const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
  const s = flying();
  expect(applyEvent(s, { type: 'future' } as unknown as LiveEvent, 0)).toBe(s);
  expect(warn).toHaveBeenCalledOnce();
  warn.mockRestore();
});

test('Kartenelemente: Liste ersetzt, Einzelnes ersetzt nach ID, Löschen entfernt, Snapshot lässt sie stehen', () => {
  const list = ev('map_elements');
  let s = applyEvent(flying(), list, 0);
  expect(Object.keys(s.mapElements)).toHaveLength(2);
  const renamed = { ...list.elements[1]!, name: 'Fundstelle' };
  s = applyEvent(s, { type: 'map_element', element: renamed }, 0);
  expect(s.mapElements[renamed.id]!.name).toBe('Fundstelle');
  expect(Object.keys(s.mapElements)).toHaveLength(2);
  const stale = { ...renamed, name: 'alt', updated_at_ms: renamed.updated_at_ms - 1 };
  expect(applyEvent(s, { type: 'map_element', element: stale }, 0)).toBe(s);
  s = applyEvent(s, { type: 'snapshot', snapshot: snapshotEmpty }, 0);
  expect(Object.keys(s.mapElements)).toHaveLength(2);
  const deleted = applyEvent(s, ev('map_element_deleted'), 0);
  expect(Object.keys(deleted.mapElements)).toEqual([renamed.id]);
  expect(applyEvent(deleted, ev('map_element_deleted'), 0)).toBe(deleted);
  expect(applyEvent(deleted, { type: 'map_elements', elements: [] }, 0).mapElements).toEqual({});
});

test('Auslösungen aus dem Strom: neue kommen dazu, dieselbe ID ersetzt die alte, ein Snapshot leert sie', () => {
  const shot = ev('photo_shot');
  const once = applyEvent(flying(), shot, 0);
  expect(Object.keys(once.photoShots)).toEqual([shot.shot.id]);
  const thumb = { ...shot.shot, photos: shot.shot.photos.map((p) => ({ ...p, thumb_url: `/x/${p.id}` })) };
  const updated = applyEvent(once, { type: 'photo_shot', shot: thumb }, 0);
  expect(updated.photoShots[shot.shot.id]!.photos[0]!.thumb_url).toBe(`/x/${thumb.photos[0]!.id}`);
  const resnap = applyEvent(updated, { type: 'snapshot', snapshot: snapshotFlying }, 0);
  // Der Snapshot lädt die Fotoliste neu; eine in der Lücke gelöschte Auslösung bliebe sonst auf der Karte.
  expect(resnap.photoShots).toEqual({});
});

test('missions_changed leert Auslösungen, letzter und laufende Flüge bleiben', () => {
  const ended = applyEvent(flying(), ev('flight_ended'), 0);
  const s = applyEvent(applyEvent(ended, ev('flight_started'), 0), ev('photo_shot'), 0);
  expect(Object.keys(s.ended)).not.toHaveLength(0);
  expect(Object.keys(s.photoShots)).not.toHaveLength(0);
  expect(Object.keys(s.flights)).not.toHaveLength(0);
  const changed = applyEvent(s, { type: 'missions_changed' }, 0);
  expect(changed.photoShots).toEqual({});
  expect(changed.ended).toBe(s.ended);
  expect(changed.flights).toBe(s.flights);
  expect(changed.mapElements).toBe(s.mapElements);
});

test('nach der Landung steht „Letzter Flug“ auch nach Einsatzänderungen noch da', () => {
  const landed = applyEvent(flying(), ev('flight_ended'), 0);
  const closed = applyEvent(landed, { type: 'missions_changed' }, 0);
  expect(closed.ended['ACSIM0001']).toBe(landed.ended['ACSIM0001']);
});
