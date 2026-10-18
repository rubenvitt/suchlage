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
  await screen.findByText(running!.name);
}

test('anlegen, laufenden abschließen, neuen starten', async () => {
  await setup();
  await userEvent.type(screen.getByLabelText('Name'), 'Vermisste Person Ebstorf');
  await userEvent.click(screen.getByRole('button', { name: 'Einsatz anlegen' }));
  expect(await group('Vorbereitet').findByText('Vermisste Person Ebstorf')).toBeInTheDocument();

  // Solange einer läuft, lehnt der Server das Starten ab.
  await userEvent.click(
    group('Vorbereitet').getByRole('button', { name: 'Starten: Vermisste Person Ebstorf' }),
  );
  expect(await screen.findByText('Es läuft schon ein Einsatz')).toBeInTheDocument();

  await userEvent.click(group('Laufend').getByRole('button', { name: /Abschließen/ }));
  expect(await screen.findByText(/wirklich abschließen/)).toBeInTheDocument();
  await userEvent.click(screen.getByRole('button', { name: 'Abschließen' }));
  expect(await group('Abgeschlossen').findByText('Übung Wietze')).toBeInTheDocument();

  await userEvent.click(
    group('Vorbereitet').getByRole('button', { name: 'Starten: Vermisste Person Ebstorf' }),
  );
  expect(await group('Laufend').findByText('Vermisste Person Ebstorf')).toBeInTheDocument();
});

test('Abschnitt anlegen und anderer Fehler zeigt die Serverantwort', async () => {
  await setup();
  server.use(
    http.post('/api/missions/:id/sections', () =>
      HttpResponse.json({ error: 'Name fehlt' }, { status: 400 }),
    ),
  );
  await userEvent.type(screen.getByLabelText('Abschnitt zu Übung Wietze'), 'Nord');
  await userEvent.click(screen.getByRole('button', { name: 'Abschnitt anlegen: Übung Wietze' }));
  expect(await screen.findByText('Name fehlt')).toBeInTheDocument();
  expect(screen.getByRole('link', { name: 'Auf der Lage öffnen: Übung Wietze' })).toHaveAttribute(
    'href',
    '/?einsatz=1',
  );
});
