import { screen } from '@testing-library/react';
import { snapshotEmpty, snapshotFlying } from '../contract/fixtures.generated';
import type { SituationSnapshot } from '../contract/generated/SituationSnapshot';
import { LiveProvider } from '../live/LiveProvider';
import { createLiveStore } from '../live/situationStore';
import { renderWithApp } from '../test/render';
import { MobileView } from './MobileView';

vi.mock('../map/SituationMap', () => ({
  SituationMap: (p: { showTools?: boolean; follow?: boolean; onFollowChange?: unknown }) => (
    <div
      data-testid="karte"
      data-tools={String(p.showTools)}
      data-follow={String(p.follow)}
      data-follow-switch={String(typeof p.onFollowChange === 'function')}
    />
  ),
}));
vi.mock('../video/whep', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../video/whep')>()),
  startWhep: vi.fn(() => new Promise(() => {})),
}));

function show(snapshot: SituationSnapshot | null) {
  const store = createLiveStore();
  if (snapshot) store.dispatch({ type: 'snapshot', snapshot });
  return renderWithApp(
    <LiveProvider store={store}>
      <MobileView />
    </LiveProvider>,
  );
}

test('mobil nur zuschauen: Livebild, kompakte Werte, Karte', async () => {
  show(snapshotFlying);
  const strip = await screen.findByRole('region', { name: 'Telemetrie' });
  expect(strip).toHaveAttribute('data-size', 'mobile');
  expect(strip).toHaveAttribute('data-freshness', 'fresh');
  expect(strip.querySelectorAll('.sl-strip-cell')).toHaveLength(8); // 2×4: sieben Werte und das Alter
  expect(screen.queryByRole('button', { name: /Livebild/ })).not.toBeInTheDocument();
  expect(screen.getByLabelText('Livebild der Drohne')).toBeInTheDocument();
});

test('mobil folgt die Karte, und wer sie verschiebt, kann das Folgen wieder einschalten', async () => {
  show(snapshotFlying);
  const map = await screen.findByTestId('karte');
  expect(map).toHaveAttribute('data-follow', 'true');
  expect(map).toHaveAttribute('data-follow-switch', 'true');
  expect(map).not.toHaveAttribute('data-tools', 'false');
});

test('vor dem ersten Stand wird die Lage abgerufen', async () => {
  show(null);
  expect(await screen.findByText('Lage wird abgerufen')).toBeInTheDocument();
  expect(screen.queryByText('Keine Drohne im Einsatz')).not.toBeInTheDocument();
});

test('ohne Fernsteuerung steht der Hinweis oben, Werte und Karte bleiben', async () => {
  show(snapshotEmpty);
  expect(await screen.findByText('Keine Drohne im Einsatz')).toBeInTheDocument();
  expect(screen.queryByLabelText('Livebild der Drohne')).not.toBeInTheDocument();
  expect(screen.getByRole('region', { name: 'Telemetrie' })).toBeInTheDocument();
  expect(screen.getByTestId('karte')).toBeInTheDocument();
});
