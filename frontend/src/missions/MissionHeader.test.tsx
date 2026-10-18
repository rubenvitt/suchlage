import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { delay, http, HttpResponse } from 'msw';
import { useLocation } from 'react-router';
import { deletedHere, queryKeys } from '../api/queries';
import { meAdmin, meKiosk, missionList } from '../contract/fixtures.generated';
import type { Mission } from '../contract/generated/Mission';
import { LiveProvider } from '../live/LiveProvider';
import { createLiveStore } from '../live/situationStore';
import { missionFixtures, missionHandlers } from '../test/handlers';
import { renderWithApp } from '../test/render';
import { server } from '../test/server';
import { SKIP_KEY } from './entry';
import { MissionHeader } from './MissionHeader';

const [running, prepared] = missionFixtures() as [Mission, Mission];
const closed: Mission = {
  ...running,
  id: 5,
  name: 'Brand Celle',
  state: 'closed',
  closed_at_ms: 1792160000000,
};

afterEach(() => {
  sessionStorage.clear();
  deletedHere.clear();
});

function Address() {
  const { pathname, search } = useLocation();
  return <output aria-label="Adresse">{pathname + search}</output>;
}

function show(route = '/', missions: Mission[] = [running, prepared]) {
  server.use(...missionHandlers(missions));
  return renderWithApp(
    <LiveProvider store={createLiveStore()}>
      <MissionHeader />
      <Address />
    </LiveProvider>,
    { route },
  );
}

const asAdmin = () => server.use(http.get('/api/me', () => HttpResponse.json(meAdmin)));
const address = () => screen.getByLabelText('Adresse');
const openMenu = async () => userEvent.click(await screen.findByRole('button', { name: /Übung Wietze/ }));

test('laufend: Name, „Laufend“ mit Dauer, aktiver Abschnitt und das Menü', async () => {
  show();
  expect(await screen.findByText('Übung Wietze', { selector: 'strong' })).toBeInTheDocument();
  expect(screen.getByText(/^Laufend · \d+:\d\d h$/)).toBeInTheDocument();
  expect(screen.getByText('· Abschnitt Nord')).toBeInTheDocument();
  const trigger = screen.getByRole('button', { name: /Übung Wietze/ });
  expect(trigger).toHaveAttribute('aria-haspopup', 'menu');
  expect(trigger).toHaveAttribute('aria-expanded', 'false');
  await openMenu();
  expect(trigger).toHaveAttribute('aria-expanded', 'true');
  const menu = within(await screen.findByRole('menu'));
  for (const item of [
    'Neuer Abschnitt …',
    'Einsatz bearbeiten',
    'Flüge dieses Einsatzes',
    'Einsatz abschließen …',
  ])
    expect(menu.getByText(item)).toBeInTheDocument();
});

test('„Neuer Abschnitt …“ legt einen Abschnitt an, er wird der aktive', async () => {
  show();
  await openMenu();
  await userEvent.click(await screen.findByText('Neuer Abschnitt …'));
  await userEvent.type(await screen.findByLabelText('Abschnitt'), 'Süd');
  await userEvent.click(screen.getByRole('button', { name: 'Anlegen' }));
  expect(await screen.findByText('· Abschnitt Süd')).toBeInTheDocument();
});

test('„Einsatz bearbeiten“ ändert Name und Ort', async () => {
  let sent: unknown = null;
  show();
  server.use(
    http.put('/api/missions/:id', async ({ request }) => {
      sent = await request.json();
      return HttpResponse.json({ ...running, name: 'Übung Wietze Süd' });
    }),
  );
  await openMenu();
  await userEvent.click(await screen.findByText('Einsatz bearbeiten'));
  const name = await screen.findByLabelText('Name');
  expect(name).toHaveValue('Übung Wietze');
  expect(screen.getByLabelText('Ort')).toHaveValue('Wietze');
  await userEvent.clear(name);
  await userEvent.type(name, 'Übung Wietze Süd');
  await userEvent.clear(screen.getByLabelText('Ort'));
  await userEvent.click(screen.getByRole('button', { name: 'Speichern' }));
  await vi.waitFor(() => expect(sent).toEqual({ name: 'Übung Wietze Süd', location: null }));
});

test('„Flüge dieses Einsatzes“ öffnet die Flugliste des Einsatzes', async () => {
  show();
  await openMenu();
  await userEvent.click(await screen.findByText('Flüge dieses Einsatzes'));
  expect(address()).toHaveTextContent('/flights?einsatz=1');
});

test('„Einsatz abschließen …“ fragt nach und führt danach auf die Startseite', async () => {
  sessionStorage.setItem(SKIP_KEY, '1');
  show('/?einsatz=1');
  await openMenu();
  await userEvent.click(await screen.findByText('Einsatz abschließen …'));
  expect(await screen.findByText(/Abgeschlossene Einsätze sind nur noch lesbar/)).toBeInTheDocument();
  await userEvent.click(screen.getByRole('button', { name: 'Abschließen' }));
  await vi.waitFor(() => expect(address()).toHaveTextContent(/^\/$/));
  expect(sessionStorage.getItem(SKIP_KEY)).toBeNull();
});

test('Abschließen während eines Flugs zeigt den Text des Servers', async () => {
  show();
  server.use(
    http.post('/api/missions/:id/close', () =>
      HttpResponse.json({ error: 'Es läuft noch ein Flug in diesem Einsatz' }, { status: 422 }),
    ),
  );
  await openMenu();
  await userEvent.click(await screen.findByText('Einsatz abschließen …'));
  await userEvent.click(await screen.findByRole('button', { name: 'Abschließen' }));
  expect(await screen.findByText('Es läuft noch ein Flug in diesem Einsatz')).toBeInTheDocument();
  expect(address()).toHaveTextContent(/^\/$/);
});

test('ohne Einsatz: deutlich, mit Löschhinweis; „Einsatz starten“ führt auf die Startseite', async () => {
  sessionStorage.setItem(SKIP_KEY, '1');
  show('/?einsatz=ohne', [prepared]);
  expect(await screen.findByText('Ohne Einsatz')).toBeInTheDocument();
  expect(screen.getByText('Zeichnungen werden nach 24 h gelöscht')).toBeInTheDocument();
  expect(screen.queryByText('Hochwasser Aller')).not.toBeInTheDocument();
  await userEvent.click(screen.getByRole('button', { name: 'Einsatz starten' }));
  expect(address()).toHaveTextContent(/^\/$/);
  expect(sessionStorage.getItem(SKIP_KEY)).toBeNull();
});

test('Vorbereitung: Name, „Einsatz starten“ startet und führt in die Lage, „Zurück“', async () => {
  show('/?einsatz=2', [{ ...running, state: 'closed' }, prepared]);
  const head = await screen.findByText(/^Vorbereitung:/);
  expect(head.parentElement).toHaveTextContent('Vorbereitung: Hochwasser Aller');
  expect(screen.getByRole('link', { name: 'Zurück' })).toHaveAttribute('href', '/');
  await userEvent.click(screen.getByRole('button', { name: 'Einsatz starten' }));
  await vi.waitFor(() => expect(address()).toHaveTextContent(/^\/$/));
  expect(await screen.findByText(/^Laufend · /)).toBeInTheDocument();
});

test('Vorbereitung: läuft schon einer, sagt die Kopfleiste das', async () => {
  show('/?einsatz=2');
  await userEvent.click(await screen.findByRole('button', { name: 'Einsatz starten' }));
  expect(await screen.findByText('Es läuft schon ein Einsatz')).toBeInTheDocument();
  expect(address()).toHaveTextContent('/?einsatz=2');
});

test('Vorbereitung: Löschen nur für die Verwaltung, danach die Einsatzseite', async () => {
  show('/?einsatz=2');
  await screen.findByText(/^Vorbereitung:/);
  expect(screen.queryByRole('button', { name: 'Löschen …' })).not.toBeInTheDocument();
});

test('Vorbereitung: die Verwaltung löscht einen vorbereiteten Einsatz', async () => {
  asAdmin();
  show('/?einsatz=2');
  await userEvent.click(await screen.findByRole('button', { name: 'Löschen …' }));
  const dialog = within(await screen.findByRole('dialog'));
  await dialog.findByText(/3 Flüge/);
  await userEvent.type(dialog.getByLabelText(/Einsatzname/), 'Hochwasser Aller');
  await userEvent.click(dialog.getByRole('button', { name: 'Endgültig löschen' }));
  await vi.waitFor(() => expect(address()).toHaveTextContent('/einsaetze'));
});

test('Archiv: nur lesen, „Flüge“ und „Zurück“, kein Löschen für die Bedienung', async () => {
  show('/?einsatz=5', [running, closed]);
  const head = await screen.findByText(/^Archiv:/);
  expect(head.parentElement).toHaveTextContent('Archiv: Brand Celle · abgeschlossen');
  expect(screen.getByRole('link', { name: 'Flüge' })).toHaveAttribute('href', '/flights?einsatz=5');
  expect(screen.getByRole('link', { name: 'Zurück' })).toHaveAttribute('href', '/einsaetze');
  expect(screen.queryByRole('button', { name: 'Löschen …' })).not.toBeInTheDocument();
  expect(screen.queryByRole('button', { name: /Brand Celle/ })).not.toBeInTheDocument();
});

test('Archiv: Löschen nennt die Mengen und verlangt den Namen', async () => {
  asAdmin();
  show('/?einsatz=5', [running, closed]);
  await userEvent.click(await screen.findByRole('button', { name: 'Löschen …' }));
  const dialog = within(await screen.findByRole('dialog'));
  expect(await dialog.findByText(/3 Flüge, 2 Videosegmente, 41 Fotos, 4 Kartenelemente/)).toBeInTheDocument();
  const confirm = dialog.getByRole('button', { name: 'Endgültig löschen' });
  expect(confirm).toBeDisabled();
  const input = dialog.getByLabelText(/Einsatzname/);
  await userEvent.type(input, 'Brand Cell');
  expect(confirm).toBeDisabled();
  await userEvent.type(input, 'e');
  expect(confirm).toBeEnabled();
  await userEvent.click(confirm);
  await vi.waitFor(() => expect(address()).toHaveTextContent('/einsaetze'));
  const missions = (await (await fetch('/api/missions')).json()) as Mission[];
  expect(missions.map((m) => m.id)).toEqual([1]);
});

test('Löschen: Einzahl und Fehler des Servers', async () => {
  asAdmin();
  show('/?einsatz=5', [running, closed]);
  server.use(
    http.get('/api/missions/:id/contents', () =>
      HttpResponse.json({ flights: 1, photos: 1, map_elements: 1, video_segments: 1 }),
    ),
    http.delete('/api/missions/:id', () =>
      HttpResponse.json({ error: 'Videoablage nicht eingehängt' }, { status: 503 }),
    ),
  );
  await userEvent.click(await screen.findByRole('button', { name: 'Löschen …' }));
  const dialog = within(await screen.findByRole('dialog'));
  expect(await dialog.findByText(/1 Flug, 1 Videosegment, 1 Foto, 1 Kartenelement/)).toBeInTheDocument();
  await userEvent.type(dialog.getByLabelText(/Einsatzname/), 'Brand Celle');
  await userEvent.click(dialog.getByRole('button', { name: 'Endgültig löschen' }));
  expect(await dialog.findByText('Videoablage nicht eingehängt')).toBeInTheDocument();
  expect(address()).toHaveTextContent('/?einsatz=5');
});

test('solange die Einsätze laden, weder Einsatz noch „Ohne Einsatz“', async () => {
  server.use(http.get('/api/missions', () => delay('infinite')));
  renderWithApp(
    <LiveProvider store={createLiveStore()}>
      <MissionHeader />
    </LiveProvider>,
  );
  expect(await screen.findByText('Einsätze werden geladen …')).toBeInTheDocument();
  expect(screen.queryByText('Ohne Einsatz')).not.toBeInTheDocument();
  expect(screen.queryAllByRole('button')).toHaveLength(0);
});

test('scheitert der Abruf, sagt die Kopfleiste das statt „Ohne Einsatz“', async () => {
  server.use(http.get('/api/missions', () => new HttpResponse(null, { status: 500 })));
  renderWithApp(
    <LiveProvider store={createLiveStore()}>
      <MissionHeader />
    </LiveProvider>,
  );
  expect(await screen.findByText('Einsätze nicht geladen')).toBeInTheDocument();
  expect(screen.queryByText('Ohne Einsatz')).not.toBeInTheDocument();
});

test('Großbild: nur der Name des laufenden Einsatzes, ohne Bedienung', async () => {
  server.use(http.get('/api/me', () => HttpResponse.json(meKiosk)));
  show('/?einsatz=2');
  expect(await screen.findByText('Übung Wietze')).toBeInTheDocument();
  expect(screen.queryByText('Hochwasser Aller')).not.toBeInTheDocument();
  expect(screen.queryAllByRole('button')).toHaveLength(0);
  expect(screen.queryAllByRole('link')).toHaveLength(0);
});

test('ohne Einsatz, während einer läuft: der Knopf führt zum laufenden', async () => {
  show('/?einsatz=ohne');
  await screen.findByText('Ohne Einsatz');
  expect(screen.queryByRole('button', { name: 'Einsatz starten' })).not.toBeInTheDocument();
  await userEvent.click(screen.getByRole('button', { name: 'Zum laufenden Einsatz' }));
  expect(address()).toHaveTextContent(/^\/$/);
});

test('verschwindet der angezeigte Einsatz, geht es zurück zum Einstieg', async () => {
  const { client } = show('/?einsatz=5', [running, closed]);
  await screen.findByText(/^Archiv:/);
  await fetch('/api/missions/5', { method: 'DELETE' });
  // So reagiert der Live-Strom auf `missions_changed`.
  await client.invalidateQueries({ queryKey: queryKeys.missions });
  await vi.waitFor(() => expect(address()).toHaveTextContent(/^\/$/));
  expect(await screen.findByText('Übung Wietze', { selector: 'strong' })).toBeInTheDocument();
});

test('eine unbekannte Einsatznummer in der Flugliste führt zur Flugliste', async () => {
  show('/flights?einsatz=99');
  await vi.waitFor(() => expect(address()).toHaveTextContent(/^\/flights$/));
});

/** Laufend, aus der Flugmappe angelegt und verknüpft; der zweite Abschnitt hat ein Kartenfoto. */
const linked = missionList.find((m) => m.origin === 'flugmappe')!;

test('verknüpft: „· Flugmappe“ im Kopf, kein „Neuer Abschnitt …“, Herkunft und Erklärung im Menü', async () => {
  show('/', [linked]);
  const note = await screen.findByText('· Flugmappe');
  expect(note).toHaveAttribute(
    'title',
    expect.stringMatching(
      /^aus der Flugmappe · mit Flugmappe verknüpft · Gruppenführer I&K · \d\d\.\d\d\., \d\d:\d\d$/,
    ),
  );
  await userEvent.click(screen.getByRole('button', { name: /Vermisste Person Waldweg/ }));
  const menu = within(await screen.findByRole('menu'));
  expect(menu.queryByText('Neuer Abschnitt …')).not.toBeInTheDocument();
  expect(menu.getByText('Abschnitte kommen aus der Flugmappe').closest('li')).toHaveAttribute(
    'aria-disabled',
    'true',
  );
  expect(
    menu.getByText(/^aus der Flugmappe · mit Flugmappe verknüpft · Gruppenführer I&K/),
  ).toBeInTheDocument();
});

test('„Kartenfotos …“ zeigt das Kartenfoto des Abschnitts mit Versionszeichen, groß im neuen Tab', async () => {
  show('/', [linked]);
  await userEvent.click(await screen.findByRole('button', { name: /Vermisste Person Waldweg/ }));
  await userEvent.click(await screen.findByText('Kartenfotos …'));
  const dialog = within(await screen.findByRole('dialog'));
  const url = '/api/missions/3/sections/5/map-photo?v=1792149100000';
  const open = dialog.getByRole('link', { name: 'Kartenfoto Verlegung Waldrand in voller Größe öffnen' });
  expect(open).toHaveAttribute('href', url);
  expect(open).toHaveAttribute('target', '_blank');
  expect(dialog.getByRole('img', { name: 'Kartenfoto Verlegung Waldrand' })).toHaveAttribute('src', url);
  // Abschnitt 1 hat kein Kartenfoto.
  expect(dialog.queryByText('Abschnitt Abschnitt 1')).not.toBeInTheDocument();
});

test('ohne Flugmappe: kein Hinweis und keine Kartenfotos im Menü', async () => {
  show();
  await openMenu();
  const menu = within(await screen.findByRole('menu'));
  expect(menu.queryByText('Kartenfotos …')).not.toBeInTheDocument();
  expect(menu.queryByText(/Flugmappe/)).not.toBeInTheDocument();
  expect(screen.queryByText('· Flugmappe')).not.toBeInTheDocument();
});
