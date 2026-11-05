import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import type { ReactNode } from 'react';
import { http, HttpResponse } from 'msw';
import { Route, Routes, useLocation } from 'react-router';
import { missionList } from '../contract/fixtures.generated';
import type { Mission } from '../contract/generated/Mission';
import { missionFixtures, missionHandlers } from '../test/handlers';
import { LiveProvider } from '../live/LiveProvider';
import { createLiveStore } from '../live/situationStore';
import { renderWithApp } from '../test/render';
import { server } from '../test/server';
import { MissionsPage } from './MissionsPage';

vi.mock('../shell/Frame', () => ({ Frame: (p: { children: ReactNode }) => <>{p.children}</> }));

function Address() {
  const { pathname, search } = useLocation();
  return <output aria-label="Adresse">{pathname + search}</output>;
}

const region = (name: string) => within(screen.getByRole('region', { name }));
const list = () => within(screen.getByRole('list', { name: 'Einsätze' }));
const rowOf = (name: string) => within(list().getByRole('link', { name }).closest('li')!);

function show(missions: Mission[]) {
  server.use(...missionHandlers(missions));
  renderWithApp(
    <LiveProvider store={createLiveStore()}>
      <Routes>
        <Route path="/einsaetze" element={<MissionsPage />} />
        <Route path="*" element={<Address />} />
      </Routes>
    </LiveProvider>,
    { route: '/einsaetze' },
  );
}

const closed = (id: number, name: string, startedAtMs: number, extra: Partial<Mission> = {}): Mission => ({
  ...missionFixtures()[0]!,
  id,
  name,
  state: 'closed',
  started_at_ms: startedAtMs,
  closed_at_ms: startedAtMs + 95 * 60_000,
  ...extra,
});

test('der laufende steht oben mit Kennzahlen und öffnet seine Seite, nicht in der Liste', async () => {
  const [running, prepared] = missionFixtures();
  show([running!, prepared!]);
  const panel = within(await screen.findByRole('region', { name: 'Laufender Einsatz' }));
  expect(panel.getByRole('link', { name: /Übung Wietze/ })).toBeInTheDocument();
  expect(await panel.findByRole('group', { name: 'Laufender Einsatz in Zahlen' })).toHaveTextContent('Flüge3');
  expect(panel.getByRole('link', { name: 'Zur Lage' })).toHaveAttribute('href', '/');
  expect(list().queryByRole('link', { name: 'Übung Wietze' })).not.toBeInTheDocument();
  await userEvent.click(panel.getByRole('link', { name: /Übung Wietze/ }));
  expect(screen.getByLabelText('Adresse')).toHaveTextContent('/einsaetze/1');
});

test('neuer Einsatz bei laufendem nur vorbereiten, danach auf seine Seite', async () => {
  const [running] = missionFixtures();
  show([running!]);
  await screen.findByRole('region', { name: 'Laufender Einsatz' });
  await userEvent.click(screen.getByRole('button', { name: 'Neuer Einsatz' }));
  const dialog = within(await screen.findByRole('dialog'));
  expect(dialog.queryByRole('button', { name: 'Anlegen und starten' })).not.toBeInTheDocument();
  await userEvent.click(dialog.getByRole('button', { name: 'Vorbereiten' }));
  expect(await dialog.findByText('Name fehlt')).toBeInTheDocument();
  await userEvent.type(dialog.getByLabelText('Einsatzname'), 'Vermisste Person Ebstorf');
  await userEvent.click(dialog.getByRole('button', { name: 'Vorbereiten' }));
  await vi.waitFor(() => expect(screen.getByLabelText('Adresse')).toHaveTextContent('/einsaetze/2'));
});

test('zweimal Enter legt nur einen Einsatz an', async () => {
  const [running] = missionFixtures();
  show([running!]);
  let posts = 0;
  server.use(
    http.post('/api/missions', async () => {
      posts += 1;
      await new Promise((r) => setTimeout(r, 50));
      return HttpResponse.json({ ...running!, id: 9, state: 'prepared', name: 'Doppelt' }, { status: 201 });
    }),
  );
  await screen.findByRole('region', { name: 'Laufender Einsatz' });
  await userEvent.click(screen.getByRole('button', { name: 'Neuer Einsatz' }));
  const dialog = within(await screen.findByRole('dialog'));
  await userEvent.type(dialog.getByLabelText('Einsatzname'), 'Doppelt{Enter}{Enter}');
  await vi.waitFor(() => expect(screen.getByLabelText('Adresse')).toHaveTextContent('/einsaetze/9'));
  expect(posts).toBe(1);
});

test('ohne laufenden: anlegen und starten führt in die Lage; „Starten“ in der Zeile startet', async () => {
  const [, prepared] = missionFixtures();
  show([prepared!]);
  expect(screen.queryByRole('region', { name: 'Laufender Einsatz' })).not.toBeInTheDocument();
  await list().findByRole('link', { name: 'Hochwasser Aller' });
  await userEvent.click(rowOf('Hochwasser Aller').getByRole('button', { name: 'Starten: Hochwasser Aller' }));
  expect(await screen.findByRole('region', { name: 'Laufender Einsatz' })).toHaveTextContent('Hochwasser Aller');
  expect(screen.queryByRole('button', { name: /^Starten:/ })).not.toBeInTheDocument();
});

test('„Anlegen und starten“ startet den neuen Einsatz und führt in die Lage', async () => {
  show([]);
  expect(await screen.findByText('Noch keine Einsätze')).toBeInTheDocument();
  await userEvent.click(screen.getByRole('button', { name: 'Neuer Einsatz' }));
  const dialog = within(await screen.findByRole('dialog'));
  await userEvent.type(dialog.getByLabelText('Einsatzname'), 'Wald');
  await userEvent.click(dialog.getByRole('button', { name: 'Anlegen und starten' }));
  await vi.waitFor(() => expect(screen.getByLabelText('Adresse')).toHaveTextContent(/^\/$/));
});

test('Archiv nach Monaten, neueste zuerst, mit Datum, Dauer und Zahlen; Zeile öffnet die Seite', async () => {
  const [, prepared] = missionFixtures();
  show([
    closed(3, 'Alt', Date.UTC(2026, 8, 20, 10)),
    closed(4, 'Neu', Date.UTC(2026, 9, 30, 10)),
    closed(5, 'Mittel', Date.UTC(2026, 9, 2, 10)),
    prepared!,
  ]);
  await list().findByRole('link', { name: 'Neu' });
  const items = list().getAllByRole('listitem'); // die Kopfzeile ist für Vorleser verborgen
  expect(items.map((li) => li.querySelector('a')?.textContent ?? li.textContent)).toEqual([
    'Vorbereitet',
    'Hochwasser Aller',
    'Oktober 2026',
    'Neu',
    'Mittel',
    'September 2026',
    'Alt',
  ]);
  const neu = rowOf('Neu');
  expect(list().getByRole('link', { name: 'Neu' })).toHaveAttribute('href', '/einsaetze/4');
  expect(neu.getByText('30.10.2026')).toBeInTheDocument();
  expect(neu.getByText('01:35 h')).toBeInTheDocument();
  expect(await neu.findByText('12')).toBeInTheDocument(); // Fotos aus den Kennzahlen (Testserver: 10 + 4·13 mod 50)
});

test('Filter und Suche', async () => {
  const [, prepared] = missionFixtures();
  show([closed(3, 'Wald Nord', Date.UTC(2026, 9, 2)), closed(4, 'Heide', Date.UTC(2026, 9, 3)), prepared!]);
  await list().findByRole('link', { name: 'Heide' });
  expect(region('Alle Einsätze').getByText('3 von 3')).toBeInTheDocument();
  await userEvent.click(screen.getByRole('radio', { name: 'Abgeschlossen 2' }));
  expect(list().queryByRole('link', { name: 'Hochwasser Aller' })).not.toBeInTheDocument();
  await userEvent.type(screen.getByRole('searchbox', { name: 'Einsätze durchsuchen' }), 'wald');
  expect(list().getAllByRole('link').map((a) => a.textContent)).toEqual(['Wald Nord']);
  await userEvent.type(screen.getByRole('searchbox', { name: 'Einsätze durchsuchen' }), 'x');
  expect(list().getByText('Keine Treffer')).toBeInTheDocument();
});

test('Kennzeichen für Flugmappe und Einsatztagebuch', async () => {
  const fm = missionList.find((m) => m.origin === 'flugmappe')!;
  show([
    { ...fm, state: 'closed', closed_at_ms: fm.started_at_ms! + 60_000 },
    closed(9, 'Nur Suchlage', Date.UTC(2026, 9, 2)),
  ]);
  const row = within((await list().findByRole('link', { name: fm.name })).closest('li')!);
  expect(row.getByText('Flugmappe')).toBeInTheDocument();
  expect(row.getByText('Einsatztagebuch')).toBeInTheDocument();
  expect(rowOf('Nur Suchlage').queryByText('Einsatztagebuch')).not.toBeInTheDocument();
});

test('„Ohne Einsatz“ führt zur Lage und zu den Flügen ohne Einsatz', async () => {
  show(missionFixtures());
  const without = within(await screen.findByRole('region', { name: 'Ohne Einsatz' }));
  expect(without.getByRole('link', { name: 'Lage ohne Einsatz' })).toHaveAttribute('href', '/?einsatz=ohne');
  expect(without.getByRole('link', { name: 'Flüge ohne Einsatz' })).toHaveAttribute('href', '/flights?einsatz=ohne');
});
