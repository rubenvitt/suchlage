import { deviceLog, snapshotFlying } from '../contract/fixtures.generated';
import type { DeviceLogEntry } from '../contract/generated/DeviceLogEntry';
import { applyEvent, initialLiveState } from '../live/reducer';
import {
  applyDeviceLog,
  defaultFilter,
  deviceName,
  initialDeviceLog,
  KEEP,
  prettyPayload,
  visibleEntries,
} from './deviceLog';

const [history, entry, missed] = deviceLog;
const base = (history as { entries: DeviceLogEntry[] }).entries[0]!;
const at = (seq: number, patch: Partial<DeviceLogEntry> = {}): DeviceLogEntry => ({ ...base, seq, ...patch });

test('Verlauf, neue Einträge und Lücken ergeben den Stand', () => {
  let s = applyDeviceLog(initialDeviceLog, history!);
  expect(s.entries.map((e) => e.seq)).toEqual([1, 2]);
  s = applyDeviceLog(s, entry!);
  expect(s.entries.map((e) => e.seq)).toEqual([1, 2, 3]);
  expect(applyDeviceLog(s, entry!)).toBe(s);
  s = applyDeviceLog(s, { type: 'entry', entry: at(7) });
  expect(s.missed).toBe(3);
  s = applyDeviceLog(s, missed!);
  expect(s.missed).toBe(7);
});

test('mehr als KEEP Einträge: die ältesten fallen heraus', () => {
  let s = initialDeviceLog;
  for (let seq = 1; seq <= KEEP + 5; seq++) s = applyDeviceLog(s, { type: 'entry', entry: at(seq) });
  expect(s.entries).toHaveLength(KEEP);
  expect(s.entries[0]!.seq).toBe(6);
});

test('Filter: neueste zuerst, Telemetrie aus, nur Probleme, Gerät und Text', () => {
  const entries = [
    at(1, { summary: 'Telemetrie', topic: 'thing/product/AC1/osd', device_sn: 'AC1' }),
    at(2, { summary: 'Befehl live_start_push', method: 'live_start_push', level: 'info' }),
    at(3, { summary: 'keine Antwort', level: 'error' }),
  ];
  expect(visibleEntries(entries, defaultFilter).map((e) => e.seq)).toEqual([3, 2]);
  expect(visibleEntries(entries, { ...defaultFilter, hideTelemetry: false })).toHaveLength(3);
  expect(visibleEntries(entries, { ...defaultFilter, onlyProblems: true }).map((e) => e.seq)).toEqual([3]);
  expect(visibleEntries(entries, { ...defaultFilter, text: 'LIVE_START' }).map((e) => e.seq)).toEqual([2]);
  expect(visibleEntries(entries, { ...defaultFilter, hideTelemetry: false, device: 'AC1' })).toHaveLength(1);
});

test('Gerätenamen kommen aus der Lage', () => {
  const live = applyEvent(initialLiveState, { type: 'snapshot', snapshot: snapshotFlying }, 0);
  const gateway = snapshotFlying.gateways[0]!;
  const aircraft = snapshotFlying.aircraft[0]!;
  expect(deviceName(gateway.sn, live)).toBe(gateway.label ?? gateway.sn);
  if (gateway.label) expect(deviceName(aircraft.sn, live)).toBe(`${gateway.label} · Fluggerät`);
  expect(deviceName('UNBEKANNT', live)).toBe('UNBEKANNT');
  expect(deviceName(null, live)).toBe('–');
});

test('Payload wird eingerückt, gekürzter bleibt Text', () => {
  expect(prettyPayload('{"a":1}')).toBe('{\n  "a": 1\n}');
  expect(prettyPayload('{"a":1…')).toBe('{"a":1…');
});
