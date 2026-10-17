import { screen, within } from '@testing-library/react';
import { http, HttpResponse } from 'msw';
import { snapshotFlying } from '../contract/fixtures.generated';
import { LiveProvider } from '../live/LiveProvider';
import { createLiveStore } from '../live/situationStore';
import { renderWithApp } from '../test/render';
import { server } from '../test/server';
import { SystemView } from './SystemPage';

test('System zeigt Broker, Mitschnitt und nicht ausgewertete Felder', () => {
  const store = createLiveStore();
  store.dispatch({ type: 'snapshot', snapshot: snapshotFlying });
  store.setStatus('open');
  renderWithApp(
    <LiveProvider store={store}>
      <SystemView />
    </LiveProvider>,
  );
  const field = (label: string) => within(screen.getByText(label).closest('[data-lfh="datenfeld"]') as HTMLElement);
  expect(field('Broker').getByText('verbunden')).toBeInTheDocument();
  expect(field('Live-Strom zum Browser').getByText('verbunden')).toBeInTheDocument();
  expect(screen.getByText('20261016T120000Z.jsonl')).toBeInTheDocument();
  expect(screen.getByText('aircraft.wind_speed_extra')).toBeInTheDocument();
  expect(screen.getByText('3')).toBeInTheDocument();
});

test('ohne Stand vom Server behauptet die Systemansicht nichts', () => {
  const store = createLiveStore();
  renderWithApp(
    <LiveProvider store={store}>
      <SystemView />
    </LiveProvider>,
  );
  expect(screen.getByText('verbindet')).toBeInTheDocument();
  expect(screen.getByText('unbekannt')).toBeInTheDocument();
  expect(screen.getByText('Noch kein Stand vom Server.')).toBeInTheDocument();
  expect(screen.queryByText('Alle empfangenen Felder sind bekannt.')).not.toBeInTheDocument();
});

test('System zeigt Version und Commit des laufenden Servers', async () => {
  renderWithApp(
    <LiveProvider store={createLiveStore()}>
      <SystemView />
    </LiveProvider>,
  );
  const field = (label: string) => within(screen.getByText(label).closest('[data-lfh="datenfeld"]') as HTMLElement);
  expect(await field('Version').findByText('0.2.3')).toBeInTheDocument();
  expect(field('Commit').getByText('a5a73ed')).toBeInTheDocument();
});

test('ein lokaler Build hat keinen Commit', async () => {
  server.use(http.get('/api/health', () => HttpResponse.json({ status: 'ok', version: '0.2.0-dev', commit: null })));
  renderWithApp(
    <LiveProvider store={createLiveStore()}>
      <SystemView />
    </LiveProvider>,
  );
  expect(await screen.findByText('0.2.0-dev')).toBeInTheDocument();
  expect(screen.getByText('lokaler Build')).toBeInTheDocument();
});
