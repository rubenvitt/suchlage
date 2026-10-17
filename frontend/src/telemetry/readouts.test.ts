import { snapshotFlying } from '../contract/fixtures.generated';
import { fmt } from './format';
import { readouts } from './readouts';

const a = snapshotFlying.aircraft[0]!;
const g = snapshotFlying.gateways[0]!;
const f = snapshotFlying.active_flights[0]!.flight;
const byKey = (r: ReturnType<typeof readouts>) => Object.fromEntries(r.map((x) => [x.key, x]));

test('Werte und Einheiten aus der Telemetrie', () => {
  const r = byKey(readouts(a, g, f, f.started_at_ms + 601_000));
  expect(r.elevation).toMatchObject({ label: 'Höhe ü. Start', value: '62', unit: 'm' });
  expect(r.speed).toMatchObject({ value: '8,4', unit: 'm/s' });
  expect(r.heading).toMatchObject({ value: '274', unit: '°' });
  expect(r.battery).toMatchObject({ value: '63', unit: '%', note: '18:20 Restflug', tone: 'neutral' });
  expect(r.gnss).toMatchObject({ value: 'RTK fix', tone: 'normal', note: '21 GPS · 18 RTK' });
  expect(r.link).toMatchObject({ value: '4/5', tone: 'neutral' });
  expect(r.flight).toMatchObject({ value: '10:01', label: 'Flugzeit' });
});

test('Akku nahe am Rückflugbedarf wird gelb, darunter rot', () => {
  const low = (percent: number) => ({
    ...a,
    telemetry: { ...a.telemetry, battery: { ...a.telemetry.battery!, percent } },
  });
  expect(byKey(readouts(low(31), g, f, 0)).battery!.tone).toBe('achtung');
  expect(byKey(readouts(low(22), g, f, 0)).battery!.tone).toBe('alarm');
});

test('schwache Funkverbindung und Rückkehr sind auffällig', () => {
  const weak = { ...g, link: { sdr_quality: 1, lte_quality: 3 } };
  const rth = { ...a, telemetry: { ...a.telemetry, mode: { code: 9, label: 'Rückkehr zum Startpunkt' } } };
  const r = byKey(readouts(rth, weak, f, 0));
  expect(r.link!.tone).toBe('alarm');
  expect(r.mode).toMatchObject({ value: 'Rückkehr zum Startpunkt', tone: 'achtung' });
});

test('ohne Fluggerät stehen Striche statt Nullen', () => {
  const r = byKey(readouts(undefined, g, undefined, 0));
  expect(r.elevation!.value).toBe('–');
  expect(r.flight!.value).toBe('–');
});

test('RTK-Fehlschlag ist rot, ohne Lagedaten bleibt die Positionierung neutral', () => {
  const failed = {
    ...a,
    telemetry: { ...a.telemetry, gnss: { ...a.telemetry.gnss!, fix: 'failed' as const } },
  };
  expect(byKey(readouts(failed, g, f, 0)).gnss).toMatchObject({ value: 'RTK gestört', tone: 'alarm' });
  expect(byKey(readouts(undefined, g, f, 0)).gnss).toMatchObject({ value: '–', tone: 'neutral' });
});

test('die Flugzeit folgt der Uhr, ein beendeter Flug zeigt seine Dauer', () => {
  const r = byKey(readouts(a, g, f, f.started_at_ms + 601_000));
  expect(r.flight!.followsClock).toBe(true);
  expect(r.heading!.followsClock).toBeUndefined();
  const ended = { ...f, ended_at_ms: f.started_at_ms + 3_725_000 };
  expect(byKey(readouts(a, g, ended, 0)).flight).toMatchObject({ label: 'Letzter Flug', value: '1:02:05' });
});

test('Kurs liegt immer in 000–359, große Abstände in Kilometern', () => {
  expect(fmt.heading(359.6)).toBe('000');
  expect(fmt.heading(-90)).toBe('270');
  expect(fmt.heading(7)).toBe('007');
  expect(fmt.distance(1_234)).toEqual(['1,2', 'km']);
  expect(fmt.distance(412)).toEqual(['412', 'm']);
  expect(fmt.signedSpeed(1.25)).toBe('+1,3');
  expect(fmt.signedSpeed(-0.4)).toBe('-0,4');
  expect(fmt.signedSpeed(-0.04)).toBe('0,0');
});
