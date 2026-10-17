import { screen } from '@testing-library/react';
import { snapshotEmpty, snapshotFlying, snapshotGatewayOnly } from '../contract/fixtures.generated';
import type { SituationSnapshot } from '../contract/generated/SituationSnapshot';
import { LiveProvider } from '../live/LiveProvider';
import { createLiveStore } from '../live/situationStore';
import { renderWithApp } from '../test/render';
import { useDichte } from '../theme/ThemeModeProvider';
import { KioskView } from './KioskView';

vi.mock('../map/SituationMap', () => ({
  SituationMap: (p: { showTools?: boolean; follow?: boolean }) => (
    <div data-testid="karte" data-tools={String(p.showTools)} data-follow={String(p.follow)} />
  ),
}));
vi.mock('../video/whep', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../video/whep')>()),
  startWhep: vi.fn(() => new Promise(() => {})),
}));

function DichteProbe() {
  return <output data-testid="dichte">{useDichte().dichte}</output>;
}

/** Vorgabeuhr des Stores: `useServerNow` rechnet auf die Serverzeit des Snapshots, die Werte sind frisch. */
function show(snapshot: SituationSnapshot | null) {
  const store = createLiveStore();
  if (snapshot) store.dispatch({ type: 'snapshot', snapshot });
  return renderWithApp(
    <LiveProvider store={store}>
      <KioskView />
      <DichteProbe />
    </LiveProvider>,
  );
}

test('Großbild hat keine Bedienelemente und folgt dem Fluggerät', async () => {
  show(snapshotFlying);
  const strip = await screen.findByRole('region', { name: 'Telemetrie' });
  expect(strip).toHaveAttribute('data-size', 'large');
  expect(strip).toHaveAttribute('data-freshness', 'fresh');
  expect(screen.queryAllByRole('button')).toHaveLength(0);
  expect(screen.queryAllByRole('link')).toHaveLength(0);
  expect(screen.getByTestId('karte')).toHaveAttribute('data-tools', 'false');
  expect(screen.getByTestId('karte')).toHaveAttribute('data-follow', 'true');
  expect(screen.getByLabelText('Livebild der Drohne')).toBeInTheDocument();
  expect(screen.getByTestId('dichte')).toHaveTextContent('komfortabel');
});

test('Großbild zeigt die Werte für den Blick aus der Entfernung', async () => {
  show(snapshotFlying);
  const strip = await screen.findByRole('region', { name: 'Telemetrie' });
  const keys = [...strip.querySelectorAll('.sl-strip-cell[data-key]')].map((c) => c.getAttribute('data-key'));
  expect(keys).toEqual(['elevation', 'speed', 'heading', 'battery', 'distance', 'mode', 'flight']);
  expect(strip.querySelector('.sl-strip-age')).toBeInTheDocument();
});

test('vor dem ersten Stand wird die Lage abgerufen', async () => {
  show(null);
  expect(await screen.findByText('Lage wird abgerufen')).toBeInTheDocument();
  expect(screen.queryByText('Keine Drohne im Einsatz')).not.toBeInTheDocument();
});

test('ohne Fernsteuerung steht ein Hinweis ohne Bedienschritte, das Livebild-Feld bleibt beschriftet', async () => {
  show(snapshotEmpty);
  expect(await screen.findByText('Keine Drohne im Einsatz')).toBeInTheDocument();
  expect(screen.queryByText(/Einstellungen/)).not.toBeInTheDocument();
  expect(screen.getByText('Ohne Fernsteuerung kein Livebild.')).toBeInTheDocument();
  expect(screen.getByTestId('karte')).toBeInTheDocument();
});

test('Fernsteuerung ohne Fluggerät', async () => {
  show(snapshotGatewayOnly);
  expect(await screen.findByText('Fluggerät aus')).toBeInTheDocument();
  expect(screen.getByText(/RC Plus 1 ist verbunden/)).toBeInTheDocument();
});
