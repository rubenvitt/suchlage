import { snapshotFlying } from '../contract/fixtures.generated';
import { aircraftCollection, boundsOf, gatewayCollection, homeCollection, trackCollection } from './layers';

const t = snapshotFlying.freshness;
const plane = snapshotFlying.aircraft[0]!;

test('Fluggerät trägt Kurs, Frische und Alter', () => {
  const fresh = aircraftCollection([plane], plane.last_seen_ms + 1_000, t);
  expect(fresh.features[0]!.properties).toMatchObject({ heading: 274, freshness: 'fresh', age: '' });
  expect(fresh.features[0]!.geometry.coordinates).toEqual([10.0134, 52.4321]);
  const delayed = aircraftCollection([plane], plane.last_seen_ms + 9_400, t);
  expect(delayed.features[0]!.properties).toMatchObject({ freshness: 'delayed', age: 'vor 9 s' });
});

test('Fluggerät ohne Position erscheint nicht', () => {
  const blind = { ...plane, telemetry: { ...plane.telemetry, position: null } };
  expect(aircraftCollection([blind], 0, t).features).toHaveLength(0);
});

test('jedes Segment wird eine eigene Linie', () => {
  const tracks = trackCollection([snapshotFlying.active_flights[0]!.segments]);
  expect(tracks.features).toHaveLength(2);
  expect(tracks.features[1]!.geometry.coordinates[0]).toEqual([10.012, 52.4315]);
});

test('ein einzelner Punkt ist noch keine Linie', () => {
  const [first] = snapshotFlying.active_flights[0]!.segments;
  expect(trackCollection([[first!.slice(0, 1)]]).features).toHaveLength(0);
});

test('Fernsteuerung mit Position wird als Trupp gezeigt und trägt ihr Alter', () => {
  const gw = snapshotFlying.gateways[0]!;
  expect(
    gatewayCollection(snapshotFlying.gateways, gw.last_seen_ms + 1_000, t).features[0]!.properties,
  ).toMatchObject({ label: 'RC Plus 1', freshness: 'fresh', age: '' });
  expect(
    gatewayCollection(snapshotFlying.gateways, gw.last_seen_ms + 45_000, t).features[0]!.properties,
  ).toMatchObject({ freshness: 'lost', age: 'vor 45 s' });
});

test('Fernsteuerung ohne Namen heißt nach ihrer Seriennummer', () => {
  const gw = { ...snapshotFlying.gateways[0]!, label: null };
  expect(gatewayCollection([gw], gw.last_seen_ms, t).features[0]!.properties.label).toBe(gw.sn);
});

test('Startpunkt steht nur da, wo er bekannt ist', () => {
  const unknown = { ...plane, telemetry: { ...plane.telemetry, home: null } };
  const homes = homeCollection([plane, unknown]);
  expect(homes.features).toHaveLength(1);
  expect(homes.features[0]!.geometry.coordinates).toEqual([10.01, 52.43]);
});

test('Umriss aller Spurpunkte', () => {
  expect(boundsOf(snapshotFlying.active_flights[0]!.segments)).toEqual([
    [10.01, 52.43],
    [10.0134, 52.4321],
  ]);
  expect(boundsOf([])).toBeNull();
});
