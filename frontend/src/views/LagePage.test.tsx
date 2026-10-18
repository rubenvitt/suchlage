import { screen, waitFor, within } from '@testing-library/react';
import { http, HttpResponse } from 'msw';
import { meKiosk, snapshotFlying } from '../contract/fixtures.generated';
import { LiveProvider } from '../live/LiveProvider';
import { createLiveStore } from '../live/situationStore';
import { renderWithApp } from '../test/render';
import { server } from '../test/server';
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
  // Der laufende Einsatz steht im Kopf, nicht mehr über der Karte.
  expect(await head.findByText('Übung Wietze', { selector: 'strong' })).toBeInTheDocument();
  expect(screen.queryByRole('combobox', { name: 'Einsatz wählen' })).not.toBeInTheDocument();
});

test('Kopf und Ansicht folgen derselben Wahl aus der Adresse', async () => {
  const head = show('/?rc=RCSIM0002');
  expect(await head.findByText('RC Plus Reserve')).toBeInTheDocument();
  expect(screen.getByText(/RC Plus Reserve ist verbunden/)).toBeInTheDocument();
});

test('die Großbild-Rolle sieht das Großbild ohne Bereichsleiste und Benutzermenü', async () => {
  server.use(http.get('/api/me', () => HttpResponse.json(meKiosk)));
  const head = show('/');
  // Die Rolle kommt mit `/api/me`; in der App hält `RequireRole` die Seite bis dahin zurück.
  await waitFor(() =>
    expect(screen.getByRole('region', { name: 'Telemetrie' })).toHaveAttribute('data-size', 'large'),
  );
  expect(head.getByText('RC Plus 1 · M350')).toBeInTheDocument();
  // Im Kopf nur der Name des laufenden Einsatzes.
  expect(await head.findByText('Übung Wietze')).toBeInTheDocument();
  expect(screen.queryByRole('navigation')).not.toBeInTheDocument();
  expect(screen.queryAllByRole('button')).toHaveLength(0);
});

test('unter 768 px die mobile Ansicht mit unterer Leiste', async () => {
  vi.stubGlobal('innerWidth', 390);
  try {
    const head = show('/');
    expect(await screen.findByRole('region', { name: 'Telemetrie' })).toHaveAttribute('data-size', 'mobile');
    expect(head.getByText('SYNC')).toBeInTheDocument();
    expect(head.queryByText(/RC Plus 1/)).not.toBeInTheDocument();
    const tabs = await screen.findByRole('navigation', { name: 'Bereiche' });
    expect(within(tabs).getByRole('link', { name: 'Flüge' })).toBeInTheDocument();
  } finally {
    vi.unstubAllGlobals();
  }
});
