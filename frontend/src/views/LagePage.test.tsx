import { screen, within } from '@testing-library/react';
import { snapshotFlying } from '../contract/fixtures.generated';
import { LiveProvider } from '../live/LiveProvider';
import { createLiveStore } from '../live/situationStore';
import { renderWithApp } from '../test/render';
import { LagePage } from './LagePage';

vi.mock('../map/SituationMap', () => ({ SituationMap: () => <div data-testid="karte" /> }));
vi.mock('../video/whep', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../video/whep')>()),
  startWhep: vi.fn(() => new Promise(() => {})),
}));

const gateway = snapshotFlying.gateways[0]!;
const spare = { ...gateway, sn: 'RCSIM0002', label: 'RC Plus Reserve', aircraft_sn: null };

function show(route: string) {
  const store = createLiveStore();
  store.dispatch({ type: 'snapshot', snapshot: { ...snapshotFlying, gateways: [gateway, spare] } });
  store.setStatus('open');
  renderWithApp(
    <LiveProvider store={store}>
      <LagePage />
    </LiveProvider>,
    { route },
  );
  return within(screen.getByRole('banner'));
}

test('der Kopf nennt die Fernsteuerung mit Fluggerät und den Live-Strom', async () => {
  const head = show('/');
  expect(await head.findByText('RC Plus 1 · M350')).toBeInTheDocument();
  expect(head.getByText('SYNC')).toBeInTheDocument();
  expect(await screen.findByRole('region', { name: 'Telemetrie' })).toBeInTheDocument();
});

test('Kopf und Ansicht folgen derselben Wahl aus der Adresse', async () => {
  const head = show('/?rc=RCSIM0002');
  expect(await head.findByText('RC Plus Reserve')).toBeInTheDocument();
  expect(screen.getByText(/RC Plus Reserve ist verbunden/)).toBeInTheDocument();
});
