import { screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { http, HttpResponse } from 'msw';
import type { RemoteControllerCreated } from '../contract/generated/RemoteControllerCreated';
import { server } from '../test/server';
import { renderWithApp } from '../test/render';
import { RemoteControllersPanel } from './RemoteControllersPanel';

test('Fehler beim Abrufen der Fernsteuerungen ist kein leeres Paneel', async () => {
  server.use(http.get('/api/remote-controllers', () => HttpResponse.json({ error: 'x' }, { status: 500 })));
  renderWithApp(<RemoteControllersPanel />);
  expect(await screen.findByText('Daten nicht abrufbar')).toBeInTheDocument();
});

test('ohne Fernsteuerung sagt das Paneel, was zu tun ist', async () => {
  server.use(http.get('/api/remote-controllers', () => HttpResponse.json([])));
  renderWithApp(<RemoteControllersPanel />);
  expect(await screen.findByText(/Noch keine Fernsteuerung/)).toBeInTheDocument();
});

test('neue Fernsteuerung zeigt die Kopplungsadresse genau einmal mit QR-Code', async () => {
  const url = 'https://suchlage.example.org/pilot#abc123';
  let body: unknown;
  const created: RemoteControllerCreated = {
    controller: { id: 3, label: 'RC Plus 2', sn: null, paired_at_ms: null },
    pairing_url: url,
  };
  server.use(
    http.post('/api/remote-controllers', async ({ request }) => {
      body = await request.json();
      return HttpResponse.json(created);
    }),
  );
  renderWithApp(<RemoteControllersPanel />);
  expect(await screen.findByText('RCSIM0001')).toBeInTheDocument();
  expect(screen.getByText('noch nicht gekoppelt')).toBeInTheDocument();
  await userEvent.type(screen.getByLabelText('Bezeichnung'), 'RC Plus 2');
  await userEvent.click(screen.getByRole('button', { name: 'Fernsteuerung hinzufügen' }));
  expect(await screen.findByText(url)).toBeInTheDocument();
  expect(body).toEqual({ label: 'RC Plus 2' });
  expect(screen.getByRole('img', { name: 'QR-Code der Adresse' })).toBeInTheDocument();
  expect(screen.getByText(/wird nur jetzt angezeigt/)).toBeInTheDocument();
  expect(screen.getByLabelText('Bezeichnung')).toHaveValue('');
});

test('Zugang widerrufen fragt nach und löscht dann genau diese Fernsteuerung', async () => {
  let deleted: string | undefined;
  server.use(
    http.delete('/api/remote-controllers/:id', ({ params }) => {
      deleted = String(params.id);
      return new HttpResponse(null, { status: 204 });
    }),
  );
  renderWithApp(<RemoteControllersPanel />);
  await userEvent.click(await screen.findByRole('button', { name: 'Zugang widerrufen: RC Plus 1' }));
  expect(await screen.findByText('Zugang von RC Plus 1 widerrufen?')).toBeInTheDocument();
  await userEvent.click(screen.getByRole('button', { name: 'Widerrufen' }));
  await vi.waitFor(() => expect(deleted).toBe('1'));
});
