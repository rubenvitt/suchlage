import { screen, within } from '@testing-library/react';
import { http, HttpResponse } from 'msw';
import type { ReactNode } from 'react';
import { flightList, missionFlights } from '../contract/fixtures.generated';
import { LiveProvider } from '../live/LiveProvider';
import { createLiveStore } from '../live/situationStore';
import userEvent from '@testing-library/user-event';
import { server } from '../test/server';
import { missionFixtures, missionHandlers } from '../test/handlers';
import { renderWithApp } from '../test/render';
import { FlightCards } from './FlightCards';
import { flightRow } from './flightRow';
import { FlightsPage } from './FlightsPage';
import { FlightTable } from './FlightTable';
import { MissionFlightCards, MissionFlightTable, missionFlightRow } from './MissionFlightList';

// Die Kopfleiste mit dem Einsatz bleibt, der übrige Rahmen nicht.
vi.mock('../shell/Frame', () => ({
  Frame: (p: { mission?: ReactNode; children: ReactNode }) => (
    <>
      {p.mission}
      {p.children}
    </>
  ),
}));

const [running, landed] = flightList as [(typeof flightList)[0], (typeof flightList)[0]];
const now = running.started_at_ms + 65_000;

// Die Standard-Antworten filtern nach Einsatz; diese Tests zählen alle drei Beispielflüge.
beforeEach(() =>
  server.use(
    http.get('/api/flights', () => HttpResponse.json(flightList)),
    http.get('/api/missions/:id/flight-log', () => HttpResponse.json(missionFlights)),
  ),
);
const emptyLog = () => server.use(http.get('/api/missions/:id/flight-log', () => HttpResponse.json([])));

const page = (route = '/') =>
  renderWithApp(
    <LiveProvider store={createLiveStore()}>
      <FlightsPage />
    </LiveProvider>,
    { route },
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

const [confirmed, byHand, inAir] = missionFlights as [
  (typeof missionFlights)[0],
  (typeof missionFlights)[0],
  (typeof missionFlights)[0],
];

test('Flugliste: Quelle in Worten, ein Flug in der Luft ohne Landung', () => {
  expect(missionFlightRow(confirmed, 0)).toMatchObject({
    duration: '18:00',
    aircraft: 'Matrice 350 RTK',
    section: 'Abschnitt 1',
    source: 'Telemetrie, bestätigt in Flugmappe Matrice 350 RTK',
  });
  expect(missionFlightRow(byHand, 0)).toMatchObject({ source: 'Flugmappe', aircraft: 'Matrice 200' });
  expect(missionFlightRow(inAir, inAir.started_at_ms + 65_000)).toMatchObject({
    end: 'in der Luft',
    duration: '01:05',
  });
  expect(missionFlightRow({ ...confirmed, confirmed_in: null, section: null }, 0)).toMatchObject({
    source: 'Telemetrie',
    section: '–',
  });
});

test('Flugliste: nur Telemetrie-Flüge führen zum Flugdetail', () => {
  renderWithApp(<MissionFlightTable flights={missionFlights} nowMs={now} />);
  const rows = screen.getAllByRole('row').slice(1);
  expect(rows).toHaveLength(3);
  expect(within(rows[0]!).getByRole('link', { name: 'Flug 6 ansehen' })).toHaveAttribute(
    'href',
    '/flights/6',
  );
  expect(within(rows[1]!).getByText('Flugmappe')).toBeInTheDocument();
  expect(within(rows[1]!).queryByRole('link')).not.toBeInTheDocument();
});

test('Flugliste mobil: eine Karte je Flug, der Handeintrag ohne Link', () => {
  renderWithApp(<MissionFlightCards flights={missionFlights} nowMs={now} />);
  const cards = screen.getAllByRole('listitem');
  expect(cards).toHaveLength(3);
  expect(within(cards[1]!).getByText('Matrice 200 · Verlegung Waldrand')).toBeInTheDocument();
  expect(within(cards[1]!).queryByRole('link')).not.toBeInTheDocument();
  expect(within(cards[2]!).getByRole('link')).toHaveAttribute('href', '/flights/7');
});

test('Liste zeigt erst den Lade-, dann den Fehlerzustand', async () => {
  server.use(
    http.get('/api/missions/:id/flight-log', () => HttpResponse.json({ error: 'x' }, { status: 500 })),
  );
  page();
  expect(screen.getByLabelText('Flüge wird geladen')).toHaveAttribute('aria-busy', 'true');
  expect(await screen.findByText('Daten nicht abrufbar')).toBeInTheDocument();
  expect(screen.getByRole('button', { name: 'Erneut abrufen' })).toBeInTheDocument();
});

test('ohne Flüge sagt die Liste, dass sie sich selbst füllt', async () => {
  emptyLog();
  page();
  expect(
    await screen.findByText(/Noch kein Flug\. Jeder Flug erscheint hier automatisch/),
  ).toBeInTheDocument();
  expect(screen.queryByRole('table')).not.toBeInTheDocument();
});

test('am Arbeitsplatz eine Tabelle mit allen Flügen des Einsatzes', async () => {
  page();
  expect(await screen.findByRole('table')).toBeInTheDocument();
  expect(screen.getByText('Übung Wietze · 3 Flüge')).toBeInTheDocument();
  expect(screen.getByRole('columnheader', { name: 'Quelle' })).toBeInTheDocument();
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

test('lädt die Flugliste des gewählten Einsatzes', async () => {
  const urls: string[] = [];
  server.use(
    http.get('/api/missions/:id/flight-log', ({ params }) => {
      urls.push(String(params.id));
      return HttpResponse.json(missionFlights.slice(0, 1));
    }),
  );
  page('/?einsatz=2');
  await vi.waitFor(() => expect(urls).toContain('2'));
  expect(await screen.findByText('Hochwasser Aller · 1 Flug')).toBeInTheDocument();
  // Ohne Zuordnung steht nichts zum Zuordnen an: Der Einsatz hat seine Flüge schon.
  expect(screen.queryByRole('combobox', { name: 'Einsatz zuordnen' })).not.toBeInTheDocument();
});

test('unter „ohne Einsatz“ lässt sich jeder Flug zuordnen', async () => {
  server.use(...missionHandlers());
  const sent: unknown[] = [];
  server.use(
    http.put('/api/flights/:id/mission', async ({ request, params }) => {
      sent.push([params.id, await request.json()]);
      return new HttpResponse(null, { status: 204 });
    }),
  );
  page('/?einsatz=ohne');
  const selects = await screen.findAllByRole('combobox', { name: 'Einsatz zuordnen' });
  expect(selects).toHaveLength(3);
  await userEvent.click(selects[1]!);
  await userEvent.click(await screen.findByTitle('Hochwasser Aller'));
  await vi.waitFor(() => expect(sent).toEqual([['6', { mission_id: 2 }]]));
});

test('leere Liste und Kopf passen zum gewählten Einsatz', async () => {
  emptyLog();
  page('/?einsatz=2');
  expect(await screen.findByText('Keine Flüge in diesem Einsatz.')).toBeInTheDocument();
  expect(screen.getByText('Hochwasser Aller · 0 Flüge')).toBeInTheDocument();
});

test('leere Liste ohne Einsatz', async () => {
  server.use(http.get('/api/flights', () => HttpResponse.json([])));
  page('/?einsatz=ohne');
  expect(await screen.findByText('Keine Flüge ohne Einsatz.')).toBeInTheDocument();
  expect(screen.getByText('ohne Einsatz · 0 aufgezeichnet')).toBeInTheDocument();
});

test('fragt die Flüge erst, wenn die Einsätze bekannt sind', async () => {
  const urls: string[] = [];
  let release!: () => void;
  const missionsKnown = new Promise<void>((r) => (release = r));
  server.use(
    http.get('/api/missions', async () => {
      await missionsKnown;
      return HttpResponse.json(missionFixtures());
    }),
    http.get('/api/flights', ({ request }) => {
      urls.push(new URL(request.url).search);
      return HttpResponse.json(flightList);
    }),
    http.get('/api/missions/:id/flight-log', ({ params }) => {
      urls.push(`log ${String(params.id)}`);
      return HttpResponse.json(missionFlights);
    }),
  );
  page();
  expect(await screen.findByText('Einsätze werden geladen …')).toBeInTheDocument();
  await new Promise((r) => setTimeout(r, 20));
  expect(urls).toEqual([]);
  release();
  await vi.waitFor(() => expect(urls).toEqual(['log 1']));
});

test('scheitern die Einsätze, meldet die Liste einen Fehler statt Flügen ohne Einsatz', async () => {
  const urls: string[] = [];
  server.use(
    http.get('/api/missions', () => new HttpResponse(null, { status: 500 })),
    http.get('/api/flights', ({ request }) => {
      urls.push(new URL(request.url).search);
      return HttpResponse.json(flightList);
    }),
    http.get('/api/missions/:id/flight-log', ({ params }) => {
      urls.push(`log ${String(params.id)}`);
      return HttpResponse.json(missionFlights);
    }),
  );
  page();
  expect(await screen.findByText('Einsätze nicht geladen')).toBeInTheDocument();
  expect(screen.getByText('Daten nicht abrufbar')).toBeInTheDocument();
  expect(urls).toEqual([]);
});
