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
