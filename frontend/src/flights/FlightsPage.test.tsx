import { screen, within } from '@testing-library/react';
import { http, HttpResponse } from 'msw';
import type { ReactNode } from 'react';
import { flightList } from '../contract/fixtures.generated';
import { LiveProvider } from '../live/LiveProvider';
import { createLiveStore } from '../live/situationStore';
import { server } from '../test/server';
import { renderWithApp } from '../test/render';
import { FlightCards } from './FlightCards';
import { flightRow } from './flightRow';
import { FlightsPage } from './FlightsPage';
import { FlightTable } from './FlightTable';

vi.mock('../shell/Frame', () => ({ Frame: (p: { children: ReactNode }) => <>{p.children}</> }));

const [running, landed] = flightList as [(typeof flightList)[0], (typeof flightList)[0]];
const now = running.started_at_ms + 65_000;

const page = () =>
  renderWithApp(
    <LiveProvider store={createLiveStore()}>
      <FlightsPage />
    </LiveProvider>,
  );

test('Zeile zeigt Dauer, Strecke, Höhe und Ausgang', () => {
  expect(flightRow(landed, 0)).toMatchObject({
    duration: '18:00',
    distance: '2,3 km',
    maxElevation: '85 m',
    outcome: 'gelandet',
    outcomeTone: 'normal',
  });
  expect(flightRow(running, now)).toMatchObject({
    outcome: 'läuft',
    outcomeTone: 'bedien',
    duration: '01:05',
  });
});

test('Zeile: abgerissene Verbindung ist Achtung, unbekannte Höhe ein Strich ohne Einheit', () => {
  const lost = { ...landed, end_reason: 'connection_lost' as const, max_elevation_m: null };
  expect(flightRow(lost, 0)).toMatchObject({
    outcome: 'Verbindung verloren',
    outcomeTone: 'achtung',
    maxElevation: '–',
  });
});

test('Tabelle verlinkt jeden Flug', () => {
  renderWithApp(<FlightTable flights={flightList} nowMs={now} />);
  const rows = screen.getAllByRole('row').slice(1);
  expect(rows).toHaveLength(3);
  expect(within(rows[0]!).getByText('läuft')).toBeInTheDocument();
  expect(within(rows[1]!).getByRole('link', { name: 'Flug 6 ansehen' })).toHaveAttribute(
    'href',
    '/flights/6',
  );
});

test('mobil eine Karte je Flug mit dem Wichtigsten und Link', () => {
  renderWithApp(<FlightCards flights={flightList} nowMs={now} />);
  const cards = screen.getAllByRole('listitem');
  expect(cards).toHaveLength(3);
  expect(within(cards[1]!).getByText('18:00')).toBeInTheDocument();
  expect(within(cards[1]!).getByText('gelandet')).toBeInTheDocument();
  expect(within(cards[1]!).getByRole('link')).toHaveAttribute('href', '/flights/6');
});

test('Liste zeigt erst den Lade-, dann den Fehlerzustand', async () => {
  server.use(http.get('/api/flights', () => HttpResponse.json({ error: 'x' }, { status: 500 })));
  page();
  expect(screen.getByLabelText('Flüge wird geladen')).toHaveAttribute('aria-busy', 'true');
  expect(await screen.findByText('Daten nicht abrufbar')).toBeInTheDocument();
  expect(screen.getByRole('button', { name: 'Erneut abrufen' })).toBeInTheDocument();
});

test('ohne Flüge sagt die Liste, dass sie sich selbst füllt', async () => {
  server.use(http.get('/api/flights', () => HttpResponse.json([])));
  page();
  expect(await screen.findByText(/Noch kein Flug aufgezeichnet/)).toBeInTheDocument();
  expect(screen.queryByRole('table')).not.toBeInTheDocument();
});

test('am Arbeitsplatz eine Tabelle mit allen Flügen', async () => {
  page();
  expect(await screen.findByRole('table')).toBeInTheDocument();
  expect(screen.getByText('3 aufgezeichnet')).toBeInTheDocument();
});

test('unter 768 px Karten statt Tabelle', async () => {
  const width = window.innerWidth;
  window.innerWidth = 360;
  try {
    page();
    expect(await screen.findAllByRole('listitem')).toHaveLength(3);
    expect(screen.queryByRole('table')).not.toBeInTheDocument();
  } finally {
    window.innerWidth = width;
  }
});
