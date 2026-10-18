import { liveEvents, snapshotFlying } from './fixtures.generated';
import type { LiveEvent } from './generated/LiveEvent';

// Record über die Union erzwingt beim Typecheck, dass hier jede Variante steht.
const ARTEN: Record<LiveEvent['type'], true> = {
  snapshot: true,
  gateway: true,
  aircraft: true,
  track_point: true,
  flight_started: true,
  flight_ended: true,
  stream: true,
  system: true,
  map_elements: true,
  map_element: true,
  map_element_deleted: true,
  photo_shot: true,
};

test('jede Live-Ereignisart hat genau ein Beispiel', () => {
  expect(liveEvents.map((e) => e.type).sort()).toEqual(Object.keys(ARTEN).sort());
});

test('Zeitstempel kommen als number, nicht als bigint', () => {
  expect(typeof snapshotFlying.server_time_ms).toBe('number');
});
