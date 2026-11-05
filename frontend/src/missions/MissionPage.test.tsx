import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { http, HttpResponse } from 'msw';
import type { ReactNode } from 'react';
import { Route, Routes, useLocation } from 'react-router';
import { meAdmin, missionList } from '../contract/fixtures.generated';
import type { Mission } from '../contract/generated/Mission';
import { LiveProvider } from '../live/LiveProvider';
import { createLiveStore } from '../live/situationStore';
import { missionFixtures, missionHandlers } from '../test/handlers';
import { renderWithApp } from '../test/render';
import { server } from '../test/server';
import { MissionPage } from './MissionPage';

vi.mock('../shell/Frame', () => ({ Frame: (p: { children: ReactNode }) => <>{p.children}</> }));

function Address() {
  const { pathname, search } = useLocation();
  return <output aria-label="Adresse">{pathname + search}</output>;
}

const asAdmin = () => server.use(http.get('/api/me', () => HttpResponse.json(meAdmin)));
const address = () => screen.getByLabelText('Adresse');
const region = (name: string) => within(screen.getByRole('region', { name }));

function show(missions: Mission[], id: number) {
  server.use(...missionHandlers(missions));
  renderWithApp(
    <LiveProvider store={createLiveStore()}>
      <Routes>
        <Route path="/einsaetze/:id" element={<MissionPage />} />
        <Route path="*" element={<Address />} />
      </Routes>
    </LiveProvider>,
    { route: `/einsaetze/${id}` },
  );
}

const [running, prepared] = missionFixtures();
const archived = (extra: Partial<Mission> = {}): Mission => ({
  ...running!,
  id: 7,
  name: 'Flächenbrand Heide',
  state: 'closed',
  started_at_ms: Date.UTC(2026, 9, 29, 11, 30),
  closed_at_ms: Date.UTC(2026, 9, 29, 14, 5),
  ...extra,
});
/** Die Zeile mit Name, Zustand und Kennzeichen. */
const titleLine = () => within(screen.getByRole('heading', { level: 1 }).parentElement!);
const openMenu = async () => {
  await userEvent.click(screen.getByRole('button', { name: 'Weitere Aktionen' }));
  return within(await screen.findByRole('menu'));
};

test('abgeschlossen: Kennzahlen, Verlauf, Pfad; „Auf der Lage ansehen“ öffnet den Einsatz', async () => {
  show([archived()], 7);
  expect(await screen.findByRole('heading', { level: 1, name: 'Flächenbrand Heide' })).toBeInTheDocument();
  expect(titleLine().getByText('Abgeschlossen')).toBeInTheDocument();
  const numbers = screen.getByRole('group', { name: 'Einsatz in Zahlen' });
  expect(numbers).toHaveTextContent('Dauer02:35h');
  expect(await within(numbers).findByText('Kartenelemente')).toBeInTheDocument();
  await vi.waitFor(() => expect(numbers).toHaveTextContent('Fotos51')) // Testserver: 10 + 7·13 mod 50;
  expect(region('Verlauf').getByText('nicht verknüpft')).toBeInTheDocument();
  expect(screen.getByRole('navigation', { name: 'Pfad' })).toHaveTextContent('Einsätze/Flächenbrand Heide');
  expect(screen.queryByRole('button', { name: 'Abschnitt anlegen' })).not.toBeInTheDocument();
  await userEvent.click(screen.getByRole('link', { name: 'Auf der Lage ansehen' }));
  expect(address()).toHaveTextContent('/?einsatz=7');
});

test('Verwaltung löscht über das Menü, nur ohne Einsatztagebuch', async () => {
  asAdmin();
  show([archived()], 7);
  await screen.findByRole('heading', { level: 1 });
  const menu = await openMenu();
  await userEvent.click(menu.getByRole('menuitem', { name: 'Einsatz löschen …' }));
  expect(await screen.findByRole('dialog', { name: 'Einsatz „Flächenbrand Heide“ löschen?' })).toBeInTheDocument();
});

test('im Einsatztagebuch verknüpft: Kennzeichen, Zeitpunkt im Verlauf, kein Löschen', async () => {
  asAdmin();
  show([archived({ etb_linked_at_ms: Date.UTC(2026, 9, 29, 15, 2) })], 7);
  await screen.findByRole('heading', { level: 1 });
  expect(titleLine().getByText('Einsatztagebuch')).toBeInTheDocument();
  expect(region('Verlauf').getByText(/^verknüpft 29\.10\.2026/)).toBeInTheDocument();
  expect(screen.queryByRole('button', { name: 'Weitere Aktionen' })).not.toBeInTheDocument();
});

test('Einsatzkraft ohne Verwaltung sieht kein Löschen', async () => {
  show([archived()], 7);
  await screen.findByRole('heading', { level: 1 });
  expect(screen.queryByRole('button', { name: 'Weitere Aktionen' })).not.toBeInTheDocument();
});

test('laufend: „Zur Lage“, Bearbeiten, Abschnitt anlegen; Abschließen bleibt auf der Seite', async () => {
  show([running!], 1);
  await screen.findByRole('heading', { level: 1, name: 'Übung Wietze' });
  expect(screen.getByRole('group', { name: 'Einsatz in Zahlen' })).toHaveTextContent('Läuft seit');

  await userEvent.click(screen.getByRole('button', { name: 'Bearbeiten' }));
  const edit = within(await screen.findByRole('dialog', { name: 'Einsatz bearbeiten' }));
  await userEvent.clear(edit.getByLabelText('Name'));
  await userEvent.type(edit.getByLabelText('Name'), 'Übung Wietze Nord');
  await userEvent.click(edit.getByRole('button', { name: 'Speichern' }));
  expect(await screen.findByRole('heading', { level: 1, name: 'Übung Wietze Nord' })).toBeInTheDocument();

  await userEvent.click(region('Abschnitte').getByRole('button', { name: 'Abschnitt anlegen' }));
  const add = within(await screen.findByRole('dialog', { name: 'Neuer Abschnitt' }));
  await userEvent.type(add.getByLabelText('Abschnitt'), 'Süd');
  await userEvent.click(add.getByRole('button', { name: 'Anlegen' }));
  expect(await region('Abschnitte').findByText('Süd')).toBeInTheDocument();

  const menu = await openMenu();
  await userEvent.click(menu.getByRole('menuitem', { name: 'Einsatz abschließen …' }));
  const close = within(await screen.findByRole('dialog', { name: /abschließen\?/ }));
  await userEvent.click(close.getByRole('button', { name: 'Abschließen' }));
  await vi.waitFor(() => expect(titleLine().getByText('Abgeschlossen')).toBeInTheDocument());
  expect(screen.queryByLabelText('Adresse')).not.toBeInTheDocument();
  await userEvent.click(screen.getByRole('link', { name: 'Auf der Lage ansehen' }));
  expect(address()).toHaveTextContent('/?einsatz=1');
});

test('vorbereitet, während ein anderer läuft: Starten gesperrt; ohne laufenden startet er in die Lage', async () => {
  show([running!, prepared!], 2);
  expect(await screen.findByRole('button', { name: 'Starten' })).toBeDisabled();
  expect(screen.getByRole('link', { name: 'Auf der Lage vorbereiten' })).toHaveAttribute('href', expect.stringContaining('einsatz='));
});

test('vorbereitet ohne laufenden: Starten führt in die Lage', async () => {
  show([prepared!], 2);
  await userEvent.click(await screen.findByRole('button', { name: 'Starten' }));
  await vi.waitFor(() => expect(address()).toHaveTextContent(/^\/$/));
});

test('aus der Flugmappe: Abschnitte mit Kartenfoto ohne „Abschnitt anlegen“, Flugmappen mit „Lösen …“', async () => {
  const fm = missionList.find((m) => m.origin === 'flugmappe')!;
  show([fm], fm.id);
  await screen.findByRole('heading', { level: 1, name: fm.name });
  const sections = region('Abschnitte');
  expect(sections.getByText('Abschnitte kommen aus der Flugmappe')).toBeInTheDocument();
  expect(sections.queryByRole('button', { name: 'Abschnitt anlegen' })).not.toBeInTheDocument();
  const url = '/api/missions/3/sections/5/map-photo?v=1792149100000';
  expect(sections.getByRole('link', { name: 'Kartenfoto Verlegung Waldrand in voller Größe öffnen' })).toHaveAttribute(
    'href',
    url,
  );
  const links = region('Flugmappen');
  expect(within(links.getByRole('list', { name: `Flugmappen von ${fm.name}` })).getAllByRole('listitem')).toHaveLength(3);
  await userEvent.click(links.getByRole('button', { name: 'Lösen …' }));
  expect(await screen.findByRole('dialog', { name: `Flugmappen · ${fm.name}` })).toBeInTheDocument();
});

test('Flüge und Fotos des Einsatzes', async () => {
  show([running!], 1);
  const flights = within(await screen.findByRole('region', { name: 'Flüge' }));
  expect(await flights.findAllByRole('link', { name: /^Flug \d+ ansehen$/ })).not.toHaveLength(0);
  expect(flights.getByRole('link', { name: 'Flugliste' })).toHaveAttribute('href', '/flights?einsatz=1');
  expect(screen.getByRole('region', { name: 'Fotos' })).toBeInTheDocument();
});

test('unbekannter Einsatz', async () => {
  show(missionFixtures(), 99);
  expect(await screen.findByRole('region', { name: 'Einsatz nicht gefunden' })).toBeInTheDocument();
});
