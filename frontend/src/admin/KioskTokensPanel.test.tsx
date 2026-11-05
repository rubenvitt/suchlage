import { screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { http, HttpResponse } from 'msw';
import type { KioskTokenCreated } from '../contract/generated/KioskTokenCreated';
import { server } from '../test/server';
import { renderWithApp } from '../test/render';
import { KioskTokensPanel } from './KioskTokensPanel';

test('neuer Großbild-Zugang zeigt den Link einmal, Widerrufen löscht den Zugang', async () => {
  const url = 'https://suchlage.example.org/kiosk#neuertoken';
  const created: KioskTokenCreated = {
    token: { id: 2, label: 'Lagezelt', created_at_ms: 1792150000000, einsatztagebuch: false },
    url,
  };
  let deleted: string | undefined;
  server.use(
    http.post('/api/kiosk-tokens', () => HttpResponse.json(created)),
    http.delete('/api/kiosk-tokens/:id', ({ params }) => {
      deleted = String(params.id);
      return new HttpResponse(null, { status: 204 });
    }),
  );
  renderWithApp(<KioskTokensPanel />);
  expect(await screen.findByText('FüKw Großbild')).toBeInTheDocument();
  await userEvent.type(screen.getByLabelText('Bezeichnung'), 'Lagezelt');
  await userEvent.click(screen.getByRole('button', { name: 'Großbild-Zugang erzeugen' }));
  expect(await screen.findByText(url)).toBeInTheDocument();
  expect(screen.getByText(/wird nur jetzt angezeigt/)).toBeInTheDocument();
  await userEvent.click(screen.getByRole('button', { name: 'Zugang widerrufen: FüKw Großbild' }));
  expect(await screen.findByText('Großbild „FüKw Großbild“ abmelden?')).toBeInTheDocument();
  await userEvent.click(screen.getByRole('button', { name: 'Widerrufen' }));
  await vi.waitFor(() => expect(deleted).toBe('1'));
});

test('ein Zugang für die IUK-Suite schickt den Haken mit', async () => {
  let body: unknown;
  server.use(
    http.post('/api/kiosk-tokens', async ({ request }) => {
      body = await request.json();
      return HttpResponse.json({
        token: { id: 2, label: 'Suite', created_at_ms: 1792150000000, einsatztagebuch: true },
        url: 'https://suchlage.example.org/kiosk#suite',
      } satisfies KioskTokenCreated);
    }),
  );
  renderWithApp(<KioskTokensPanel />);
  await screen.findByText('FüKw Großbild');
  expect(screen.queryByText('IUK-Suite')).not.toBeInTheDocument();
  await userEvent.type(screen.getByLabelText('Bezeichnung'), 'Suite');
  await userEvent.click(screen.getByRole('checkbox', { name: 'Für die IUK-Suite (Einsatztagebuch)' }));
  await userEvent.click(screen.getByRole('button', { name: 'Großbild-Zugang erzeugen' }));
  await vi.waitFor(() => expect(body).toEqual({ label: 'Suite', einsatztagebuch: true }));
});
