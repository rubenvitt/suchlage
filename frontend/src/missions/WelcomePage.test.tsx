import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { http, HttpResponse } from 'msw';
import { useLocation } from 'react-router';
import { queryKeys } from '../api/queries';
import { meKiosk, snapshotFlying } from '../contract/fixtures.generated';
import { LiveProvider } from '../live/LiveProvider';
import { createLiveStore } from '../live/situationStore';
import { missionFixtures, missionHandlers } from '../test/handlers';
import { renderWithApp } from '../test/render';
import { server } from '../test/server';
import { EntryPage } from './WelcomePage';

vi.mock('../map/SituationMap', () => ({ SituationMap: () => <div data-testid="karte" /> }));
vi.mock('../video/whep', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../video/whep')>()),
  startWhep: vi.fn(() => new Promise(() => {})),
}));

const [running, prepared] = missionFixtures();

afterEach(() => sessionStorage.clear());

function Address() {
  const { search } = useLocation();
  return <output aria-label="Adresse">{search}</output>;
}

/** Einstieg mit den Einsätzen `missions` auf dem Server. */
function show(missions = [prepared!], route = '/') {
  server.use(...missionHandlers(missions));
  const store = createLiveStore();
  store.dispatch({ type: 'snapshot', snapshot: snapshotFlying });
  store.setStatus('open');
  return renderWithApp(
    <LiveProvider store={store}>
      <EntryPage />
      <Address />
    </LiveProvider>,
    { route },
  );
}

const welcome = () => screen.findByRole('heading', { name: 'Kein Einsatz läuft' });
/** Der Einsatz in der Leiste der Lage. */
const lageOf = (name: string) => screen.findByText(name, { selector: '.sl-mission-status strong' });

test('läuft ein Einsatz, öffnet sich direkt seine Lage', async () => {
  show([running!, prepared!]);
  expect(await lageOf('Übung Wietze')).toBeInTheDocument();
  expect(screen.queryByRole('heading', { name: 'Kein Einsatz läuft' })).not.toBeInTheDocument();
});

test('„Einsatz starten“ legt an, startet in einem Schritt und zeigt die Lage', async () => {
  show();
  await welcome();
  await userEvent.type(screen.getByLabelText('Einsatzname'), 'Vermisste Person Ebstorf');
  await userEvent.type(screen.getByLabelText('Ort (optional)'), 'Hanstedt');
  await userEvent.click(screen.getByRole('button', { name: 'Einsatz starten' }));
  expect(await lageOf('Vermisste Person Ebstorf')).toBeInTheDocument();
  expect(screen.getByText(/^Laufend · /)).toBeInTheDocument();
  expect(screen.queryByRole('heading', { name: 'Kein Einsatz läuft' })).not.toBeInTheDocument();
});

test('ohne Namen startet nichts', async () => {
  show();
  await welcome();
  await userEvent.click(screen.getByRole('button', { name: 'Einsatz starten' }));
  expect(await screen.findByText('Name fehlt')).toBeInTheDocument();
});

test('läuft inzwischen ein anderer, sagt die Seite das und zeigt dessen Lage', async () => {
  show();
  await welcome();
  // Ein anderer Platz startet den vorbereiteten Einsatz, bevor die Liste hier neu lädt.
  await fetch('/api/missions/2/start', { method: 'POST' });
  await userEvent.type(screen.getByLabelText('Einsatzname'), 'Vermisste Person Ebstorf');
  await userEvent.click(screen.getByRole('button', { name: 'Einsatz starten' }));
  expect(await screen.findByText(/Es läuft schon ein Einsatz/)).toBeInTheDocument();
  expect(await lageOf('Hochwasser Aller')).toBeInTheDocument();
});

test('ein anderer Fehler bleibt auf der Startseite stehen', async () => {
  show();
  server.use(
    http.post('/api/missions', () => HttpResponse.json({ error: 'Name zu lang' }, { status: 400 })),
  );
  await welcome();
  await userEvent.type(screen.getByLabelText('Einsatzname'), 'x');
  await userEvent.click(screen.getByRole('button', { name: 'Einsatz starten' }));
  expect(await screen.findByText('Name zu lang')).toBeInTheDocument();
  expect(screen.getByRole('heading', { name: 'Kein Einsatz läuft' })).toBeInTheDocument();
});

test('scheitert nur der Start, steht der Einsatz unter „Vorbereitet“ und ein zweiter Klick legt keinen an', async () => {
  show();
  server.use(
    http.post('/api/missions/:id/start', () =>
      HttpResponse.json({ error: 'Datenbank ist gerade beschäftigt' }, { status: 503 }),
    ),
  );
  const creates: string[] = [];
  const count = ({ request }: { request: Request }) => {
    if (request.method === 'POST' && new URL(request.url).pathname === '/api/missions') creates.push(request.url);
  };
  server.events.on('request:start', count);
  try {
    await welcome();
    await userEvent.type(screen.getByLabelText('Einsatzname'), 'Vermisste Person Ebstorf');
    await userEvent.click(screen.getByRole('button', { name: 'Einsatz starten' }));
    expect(
      await screen.findByText(
        '„Vermisste Person Ebstorf“ ist angelegt, aber nicht gestartet – unter ‚Vorbereitet‘ starten. Datenbank ist gerade beschäftigt',
      ),
    ).toBeInTheDocument();
    expect(screen.getByLabelText('Einsatzname')).toHaveValue('');
    expect(
      await within(screen.getByRole('region', { name: 'Vorbereitet' })).findByRole('link', {
        name: 'Vermisste Person Ebstorf',
      }),
    ).toBeInTheDocument();
    await userEvent.click(screen.getByRole('button', { name: 'Einsatz starten' }));
    expect(await screen.findByText('Name fehlt')).toBeInTheDocument();
    expect(creates).toHaveLength(1);
  } finally {
    server.events.removeListener('request:start', count);
  }
});

test('„Nur vorbereiten“ öffnet die Lage des neuen Einsatzes über die Adresse', async () => {
  show();
  await welcome();
  await userEvent.type(screen.getByLabelText('Einsatzname'), 'Vermisste Person Ebstorf');
  await userEvent.click(screen.getByRole('button', { name: 'Nur vorbereiten' }));
  expect(await lageOf('Vermisste Person Ebstorf')).toBeInTheDocument();
  expect(screen.getByLabelText('Adresse')).toHaveTextContent('?einsatz=3');
  expect(screen.getByText('Vorbereitung:')).toBeInTheDocument();
});

test('vorbereitete Einsätze: Name öffnet die Lage, „Starten“ startet, dazu der Weg ins Archiv', async () => {
  const closed = { ...running!, id: 5, name: 'Alt', state: 'closed' as const };
  show([prepared!, closed]);
  const list = within(await screen.findByRole('region', { name: 'Vorbereitet' }));
  expect(list.getByRole('link', { name: 'Hochwasser Aller' })).toHaveAttribute('href', '/?einsatz=2');
  expect(list.queryByText('Alt')).not.toBeInTheDocument();
  expect(screen.getByRole('link', { name: 'Archiv öffnen (1)' })).toHaveAttribute('href', '/einsaetze');
  await userEvent.click(list.getByRole('button', { name: 'Starten: Hochwasser Aller' }));
  expect(await lageOf('Hochwasser Aller')).toBeInTheDocument();
  expect(screen.getByText(/^Laufend · /)).toBeInTheDocument();
});

test('„Überspringen“ zeigt die Lage ohne Einsatz; startet anderswo einer, folgt sie ihm', async () => {
  const { client } = show();
  await welcome();
  expect(screen.getByText('Zeichnungen ohne Einsatz werden nach 24 Stunden gelöscht.')).toBeInTheDocument();
  await userEvent.click(screen.getByRole('button', { name: 'Überspringen' }));
  expect(await screen.findByText('Ohne Einsatz')).toBeInTheDocument();
  expect(screen.getByLabelText('Adresse')).toHaveTextContent('');

  await fetch('/api/missions/2/start', { method: 'POST' });
  // So reagiert der Live-Strom auf `missions_changed` (LiveProvider.test).
  await client.invalidateQueries({ queryKey: queryKeys.missions });
  expect(await lageOf('Hochwasser Aller')).toBeInTheDocument();
  expect(screen.queryByRole('heading', { name: 'Kein Einsatz läuft' })).not.toBeInTheDocument();
});

test('übersprungen bleibt übersprungen, auch nach dem Neuladen', async () => {
  sessionStorage.setItem('suchlage:ohne-einsatz', '1');
  show();
  expect(await screen.findByText('Ohne Einsatz')).toBeInTheDocument();
});

test('eine Adresse mit Einsatz öffnet die Lage, auch ohne laufenden', async () => {
  show([prepared!], '/?einsatz=2');
  expect(await lageOf('Hochwasser Aller')).toBeInTheDocument();
});

test('das Großbild zeigt immer die Lage, nie die Startseite', async () => {
  server.use(http.get('/api/me', () => HttpResponse.json(meKiosk)));
  show();
  expect(await screen.findByRole('region', { name: 'Telemetrie' })).toBeInTheDocument();
  expect(screen.queryByRole('button', { name: 'Einsatz starten' })).not.toBeInTheDocument();
});
