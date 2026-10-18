import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { http, HttpResponse } from 'msw';
import type { ReactNode } from 'react';
import { missionHandlers, missionFixtures } from '../test/handlers';
import { server } from '../test/server';
import { renderWithApp } from '../test/render';
import { MissionsPage } from './MissionsPage';

vi.mock('../shell/Frame', () => ({ Frame: (p: { children: ReactNode }) => <>{p.children}</> }));

const group = (name: string) => within(screen.getByRole('region', { name }));

async function setup() {
  const [running, prepared] = missionFixtures();
  server.use(...missionHandlers([running!, prepared!]));
  renderWithApp(<MissionsPage />);
  await screen.findByText(prepared!.name);
}

test('anlegen, starten scheitert, solange einer läuft; ohne laufenden startet er', async () => {
  await setup();
  // Der laufende steht in der Kopfleiste, nicht in der Übersicht.
  expect(screen.queryByRole('region', { name: 'Laufend' })).not.toBeInTheDocument();
  await userEvent.type(screen.getByLabelText('Name'), 'Vermisste Person Ebstorf');
  await userEvent.click(screen.getByRole('button', { name: 'Einsatz anlegen' }));
  expect(await group('Vorbereitet').findByText('Vermisste Person Ebstorf')).toBeInTheDocument();

  await userEvent.click(
    group('Vorbereitet').getByRole('button', { name: 'Starten: Vermisste Person Ebstorf' }),
  );
  expect(await screen.findByText('Es läuft schon ein Einsatz')).toBeInTheDocument();

  await fetch('/api/missions/1/close', { method: 'POST' });
  await userEvent.click(
    group('Vorbereitet').getByRole('button', { name: 'Starten: Vermisste Person Ebstorf' }),
  );
  await vi.waitFor(() =>
    expect(group('Vorbereitet').queryByText('Vermisste Person Ebstorf')).not.toBeInTheDocument(),
  );
  expect(group('Archiv').getByText('Übung Wietze')).toBeInTheDocument();
});

test('Archiv: abgeschlossene, die neuesten zuerst, jeweils zum Öffnen', async () => {
  const [running] = missionFixtures();
  const closedAt = (id: number, name: string, closed_at_ms: number) => ({
    ...running!,
    id,
    name,
    state: 'closed' as const,
    closed_at_ms,
  });
  server.use(
    ...missionHandlers([
      closedAt(3, 'Alt', 1792100000000),
      closedAt(4, 'Neu', 1792200000000),
      closedAt(5, 'Mittel', 1792150000000),
    ]),
  );
  renderWithApp(<MissionsPage />);
  await screen.findByText('Neu');
  const names = screen.getByRole('region', { name: 'Archiv' }).querySelectorAll('strong');
  expect([...names].map((e) => e.textContent)).toEqual(['Neu', 'Mittel', 'Alt']);
  expect(group('Archiv').getByRole('link', { name: 'Auf der Lage öffnen: Neu' })).toHaveAttribute(
    'href',
    '/?einsatz=4',
  );
});

test('Abschnitt anlegen und anderer Fehler zeigt die Serverantwort', async () => {
  await setup();
  server.use(
    http.post('/api/missions/:id/sections', () =>
      HttpResponse.json({ error: 'Name fehlt' }, { status: 400 }),
    ),
  );
  await userEvent.type(screen.getByLabelText('Abschnitt zu Hochwasser Aller'), 'Nord');
  await userEvent.click(screen.getByRole('button', { name: 'Abschnitt anlegen: Hochwasser Aller' }));
  expect(await screen.findByText('Name fehlt')).toBeInTheDocument();
  expect(screen.getByRole('link', { name: 'Auf der Lage öffnen: Hochwasser Aller' })).toHaveAttribute(
    'href',
    '/?einsatz=2',
  );
});

test('409 bei einer anderen Aktion als Starten zeigt die Serverantwort', async () => {
  await setup();
  server.use(
    http.post('/api/missions/:id/sections', () =>
      HttpResponse.json({ error: 'Einsatz ist nicht mehr offen' }, { status: 409 }),
    ),
  );
  await userEvent.type(screen.getByLabelText('Abschnitt zu Hochwasser Aller'), 'Nord');
  await userEvent.click(screen.getByRole('button', { name: 'Abschnitt anlegen: Hochwasser Aller' }));
  expect(await screen.findByText('Einsatz ist nicht mehr offen')).toBeInTheDocument();
  expect(screen.queryByText('Es läuft schon ein Einsatz')).not.toBeInTheDocument();
});

test('nach erfolgreichem Anlegen ist das Abschnittsfeld leer', async () => {
  await setup();
  const input = screen.getByLabelText('Abschnitt zu Hochwasser Aller');
  await userEvent.type(input, 'Süd');
  await userEvent.click(screen.getByRole('button', { name: 'Abschnitt anlegen: Hochwasser Aller' }));
  expect(await screen.findByText('Süd')).toBeInTheDocument();
  await vi.waitFor(() => expect(screen.getByLabelText('Abschnitt zu Hochwasser Aller')).toHaveValue(''));
});

test('„Ohne Einsatz“ führt zur Lage und zu den Flügen ohne Einsatz', async () => {
  await setup();
  expect(group('Ohne Einsatz').getByRole('link', { name: 'Lage ohne Einsatz' })).toHaveAttribute(
    'href',
    '/?einsatz=ohne',
  );
  expect(group('Ohne Einsatz').getByRole('link', { name: 'Flüge ohne Einsatz' })).toHaveAttribute(
    'href',
    '/flights?einsatz=ohne',
  );
});
