import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import {
  liveEvents,
  snapshotEmpty,
  snapshotFlying,
  snapshotGatewayOnly,
} from '../contract/fixtures.generated';
import type { LiveEvent } from '../contract/generated/LiveEvent';
import type { SituationSnapshot } from '../contract/generated/SituationSnapshot';
import { LiveProvider } from '../live/LiveProvider';
import { createLiveStore } from '../live/situationStore';
import { SyncCell } from '../shell/SyncCell';
import { renderWithApp } from '../test/render';
import { WorkstationView } from './WorkstationView';

vi.mock('../map/SituationMap', () => ({ SituationMap: () => <div data-testid="karte" /> }));
vi.mock('../video/whep', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../video/whep')>()),
  startWhep: vi.fn(() => new Promise(() => {})),
}));

const eventOf = <T extends LiveEvent['type']>(type: T) =>
  liveEvents.find((e): e is Extract<LiveEvent, { type: T }> => e.type === type)!;

/**
 * Die Store-Uhr ist die echte Browseruhr: So rechnet `useServerNow` auf die Serverzeit des
 * Snapshots, und die Beispieltelemetrie ist frisch wie im Einsatz.
 */
function show(
  snapshot: SituationSnapshot | null,
  extra: LiveEvent[] = [],
  status: 'open' | 'reconnecting' = 'open',
) {
  const store = createLiveStore();
  if (snapshot) store.dispatch({ type: 'snapshot', snapshot });
  extra.forEach(store.dispatch);
  store.setStatus(status);
  return renderWithApp(
    <LiveProvider store={store}>
      <SyncCell />
      <WorkstationView />
    </LiveProvider>,
  );
}

test('vor dem ersten Stand wird die Lage abgerufen, nicht als leer gemeldet', async () => {
  show(null);
  expect(await screen.findByText('Lage wird abgerufen')).toBeInTheDocument();
  expect(screen.queryByText('Keine Fernsteuerung verbunden')).not.toBeInTheDocument();
  expect(screen.getByTestId('karte')).toBeInTheDocument();
});

test('ohne Fernsteuerung erklärt die Ansicht den nächsten Schritt', async () => {
  show(snapshotEmpty);
  expect(await screen.findByText('Keine Fernsteuerung verbunden')).toBeInTheDocument();
  expect(screen.getByText(/Einstellungen → Fernsteuerungen/)).toBeInTheDocument();
  expect(screen.getByTestId('karte')).toBeInTheDocument();
  expect(screen.queryByText('Am Boden')).not.toBeInTheDocument();
});

test('Fernsteuerung ohne Fluggerät', async () => {
  show(snapshotGatewayOnly);
  expect(await screen.findByText('Fluggerät aus')).toBeInTheDocument();
  expect(screen.getByText(/RC Plus 1 ist verbunden/)).toBeInTheDocument();
  expect(screen.queryByText('Am Boden')).not.toBeInTheDocument();
});

test('laufender Flug zeigt frische Leiste, Flugpaneel und Livebild', async () => {
  show(snapshotFlying);
  const strip = await screen.findByRole('region', { name: 'Telemetrie' });
  expect(strip).toHaveAttribute('data-freshness', 'fresh');
  const flight = screen.getByRole('region', { name: 'Flug läuft' });
  expect(within(flight).getByText('Flugzeit')).toBeInTheDocument();
  expect(within(flight).getByText('Strecke')).toBeInTheDocument();
  expect(within(flight).getByText('max. Höhe')).toBeInTheDocument();
  expect(screen.getByLabelText('Livebild der Drohne')).toBeInTheDocument();
  expect(screen.queryByText('Keine Fernsteuerung verbunden')).not.toBeInTheDocument();
  expect(screen.queryByText('Fluggerät aus')).not.toBeInTheDocument();
});

test('am Boden zeigt das Flugpaneel den letzten Flug', async () => {
  const aircraft = snapshotFlying.aircraft[0]!;
  show(snapshotFlying, [
    eventOf('flight_ended'),
    { type: 'aircraft', aircraft: { ...aircraft, phase: 'ground', flight_id: null } },
  ]);
  const panel = await screen.findByRole('region', { name: 'Am Boden' });
  expect(within(panel).getByText('Dauer')).toBeInTheDocument();
  expect(within(panel).getByText(/^beendet /)).toBeInTheDocument();
  expect(screen.queryByText('Flug läuft')).not.toBeInTheDocument();
});

test('Videofehler lässt Karte und Telemetrie weiterlaufen', async () => {
  show(snapshotFlying, [eventOf('stream')]);
  expect(await screen.findByText(/513011/)).toBeInTheDocument();
  expect(screen.getByRole('region', { name: 'Telemetrie' })).toHaveAttribute('data-freshness', 'fresh');
  expect(screen.getByTestId('karte')).toBeInTheDocument();
});

test('getrennter Live-Strom ist im Kopf sichtbar', async () => {
  show(snapshotFlying, [], 'reconnecting');
  expect(await screen.findByText('GETRENNT')).toBeInTheDocument();
});

test('Livebild groß tauscht die Spalten und lässt sich zurücknehmen', async () => {
  const { container } = show(snapshotFlying);
  const swap = await screen.findByRole('button', { name: 'Livebild groß' });
  expect(swap).toHaveAttribute('aria-pressed', 'false');
  await userEvent.click(swap);
  expect(swap).toHaveAttribute('aria-pressed', 'true');
  expect(container.querySelector('.sl-work')).toHaveAttribute('data-swap', 'true');
  await userEvent.click(swap);
  expect(container.querySelector('.sl-work')).toHaveAttribute('data-swap', 'false');
});

test('mit zwei Fernsteuerungen lässt sich die gezeigte wählen', async () => {
  const gateway = snapshotFlying.gateways[0]!;
  const spare = { ...gateway, sn: 'RCSIM0002', label: 'RC Plus Reserve', aircraft_sn: null };
  show({ ...snapshotFlying, gateways: [gateway, spare] });
  const choice = await screen.findByRole('radiogroup', { name: 'Fernsteuerung wählen' });
  await userEvent.click(within(choice).getByRole('radio', { name: 'RC Plus Reserve' }));
  expect(await screen.findByText('Fluggerät aus')).toBeInTheDocument();
  expect(screen.getByText(/RC Plus Reserve ist verbunden/)).toBeInTheDocument();
});
