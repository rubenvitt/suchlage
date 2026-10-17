import { snapshotFlying, snapshotGatewayOnly } from '../contract/fixtures.generated';
import type { GatewayState } from '../contract/generated/GatewayState';
import type { SituationSnapshot } from '../contract/generated/SituationSnapshot';
import { applyEvent, initialLiveState } from '../live/reducer';
import { focusOf, gatewayName } from './selectors';

const state = (snap: SituationSnapshot) =>
  applyEvent(initialLiveState, { type: 'snapshot', snapshot: snap }, snap.server_time_ms);

const flyingGateway = snapshotFlying.gateways[0]!;
/** Eine zweite, später gesehene Fernsteuerung ohne Fluggerät. */
const spare: GatewayState = {
  ...flyingGateway,
  sn: 'RCSIM0002',
  label: 'RC Plus Reserve',
  aircraft_sn: null,
  last_seen_ms: flyingGateway.last_seen_ms + 5_000,
};
const twoGateways: SituationSnapshot = { ...snapshotFlying, gateways: [flyingGateway, spare] };

test('Fokus verbindet Fernsteuerung, Fluggerät, Flug und Stream', () => {
  const f = focusOf(state(snapshotFlying));
  expect(f.gateway?.sn).toBe('RCSIM0001');
  expect(f.aircraft?.sn).toBe('ACSIM0001');
  expect(f.flight?.flight.id).toBe(7);
  expect(f.stream?.phase).toBe('live');
});

test('Fernsteuerung ohne Fluggerät hat keinen Flug', () => {
  const f = focusOf(state(snapshotGatewayOnly));
  expect(f.gateway?.sn).toBe('RCSIM0001');
  expect(f.aircraft).toBeUndefined();
  expect(f.flight).toBeUndefined();
});

test('ohne Wahl gewinnt die Fernsteuerung mit Fluggerät vor einer später gesehenen ohne', () => {
  expect(focusOf(state(twoGateways)).gateway?.sn).toBe('RCSIM0001');
});

test('ohne Fluggerät gilt die zuletzt gesehene Fernsteuerung', () => {
  const both = { ...twoGateways, gateways: [{ ...flyingGateway, aircraft_sn: null }, spare] };
  expect(focusOf(state(both)).gateway?.sn).toBe('RCSIM0002');
});

test('eine gewählte Fernsteuerung gilt, eine unbekannte Wahl fällt auf die Vorgabe zurück', () => {
  const s = state(twoGateways);
  const chosen = focusOf(s, 'RCSIM0002');
  expect(chosen.gateway?.sn).toBe('RCSIM0002');
  expect(chosen.aircraft).toBeUndefined();
  expect(focusOf(s, 'RCGIBTESNICHT').gateway?.sn).toBe('RCSIM0001');
});

test('ohne Lage gibt es keinen Fokus', () => {
  expect(focusOf(initialLiveState)).toEqual({});
});

test('Name der Fernsteuerung: Beschriftung aus der Verwaltung, sonst Seriennummer', () => {
  expect(gatewayName(flyingGateway)).toBe('RC Plus 1');
  expect(gatewayName({ ...flyingGateway, label: null })).toBe('RCSIM0001');
});
