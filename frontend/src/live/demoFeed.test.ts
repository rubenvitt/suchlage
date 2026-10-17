import { snapshotFlying } from '../contract/fixtures.generated';
import type { LiveEvent } from '../contract/generated/LiveEvent';
import { demoFeed, DEMO_GAP } from './demoFeed';

const T0 = 1_800_000_000_000;
type Of<T extends LiveEvent['type']> = Extract<LiveEvent, { type: T }>;

afterEach(() => vi.useRealTimers());

test('Vorschau beginnt mit einem Snapshot auf der aktuellen Uhr und schweigt in der Lücke', () => {
  vi.useFakeTimers({ now: T0 });
  const events: LiveEvent[] = [];
  const statuses: string[] = [];
  const stop = demoFeed({ url: '/api/live', onEvent: (e) => events.push(e), onStatus: (s) => statuses.push(s) });
  expect(statuses).toEqual(['open']);
  const snap = (events[0] as Of<'snapshot'>).snapshot;
  expect(snap.server_time_ms).toBe(T0);
  expect(snap.aircraft[0]!.last_seen_ms).toBe(T0);
  // Die Spur endet wie im Original kurz vor „jetzt“, nur auf die aktuelle Uhr verschoben.
  const shift = T0 - snapshotFlying.server_time_ms;
  expect(snap.active_flights[0]!.segments[1]!.at(-1)!.t_ms).toBe(
    snapshotFlying.active_flights[0]!.segments[1]!.at(-1)!.t_ms + shift,
  );

  vi.advanceTimersByTime(DEMO_GAP.from_s * 1000);
  const before = events.length;
  expect(events.filter((e) => e.type === 'track_point')).toHaveLength(DEMO_GAP.from_s - 1);
  vi.advanceTimersByTime((DEMO_GAP.to_s - DEMO_GAP.from_s) * 1000 - 1);
  expect(events.length).toBe(before);
  vi.advanceTimersByTime(1_001);
  const resumed = events.at(-1)!;
  expect(resumed.type === 'track_point' && resumed.segment).toBe(2);
  stop();
  vi.advanceTimersByTime(5_000);
  expect(events.at(-1)).toBe(resumed);
});

test('Fluggerät und Fernsteuerung melden sich jede Sekunde, der Kurs folgt der Bahn', () => {
  vi.useFakeTimers({ now: T0 });
  const events: LiveEvent[] = [];
  const stop = demoFeed({ url: '/api/live', onEvent: (e) => events.push(e), onStatus: () => {} });
  vi.advanceTimersByTime(2_000);
  stop();
  const planes = events.filter((e): e is Of<'aircraft'> => e.type === 'aircraft').map((e) => e.aircraft);
  const gateways = events.filter((e): e is Of<'gateway'> => e.type === 'gateway').map((e) => e.gateway);
  expect(planes.map((p) => p.last_seen_ms)).toEqual([T0 + 1_000, T0 + 2_000]);
  expect(gateways.map((g) => g.last_seen_ms)).toEqual([T0 + 1_000, T0 + 2_000]);
  const [a, b] = planes.map((p) => p.telemetry.position!);
  const east = (b!.lon - a!.lon) * Math.cos((a!.lat * Math.PI) / 180);
  const bearing = (Math.atan2(east, b!.lat - a!.lat) * 180) / Math.PI;
  // Sehne zwischen zwei Sekunden gegen den Kurs am Ende: auf dem Kreis wenige Grad Unterschied.
  expect(Math.abs(planes[1]!.telemetry.heading_deg! - bearing)).toBeLessThan(3);
});
