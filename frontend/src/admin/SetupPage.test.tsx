import { screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { http, HttpResponse } from 'msw';
import { authOptionsOidc } from '../contract/fixtures.generated';
import { server } from '../test/server';
import { renderWithApp } from '../test/render';
import { SetupPage } from './SetupPage';

afterEach(() => {
  window.location.hash = '';
});

test('Einrichtung sendet Token und Pocket-ID-Daten und zeigt die Callback-Adresse des Servers', async () => {
  window.location.hash = '#einrichtungstoken';
  let body: unknown;
  server.use(
    http.post('/api/setup', async ({ request }) => {
      body = await request.json();
      return HttpResponse.json(
        { error: 'Anmeldedienst nicht erreichbar oder Issuer falsch' },
        { status: 503 },
      );
    }),
  );
  renderWithApp(<SetupPage />, { route: '/einrichtung' });
  expect(await screen.findByText(authOptionsOidc.redirect_url)).toBeInTheDocument();
  await userEvent.type(screen.getByLabelText('Issuer'), 'https://id.example.org');
  await userEvent.type(screen.getByLabelText('Client-ID'), 'suchlage');
  await userEvent.type(screen.getByLabelText('Client-Secret'), 'geheim');
  await userEvent.click(screen.getByRole('button', { name: 'Einrichten und anmelden' }));
  expect(await screen.findByText('Anmeldedienst nicht erreichbar oder Issuer falsch')).toBeInTheDocument();
  expect(body).toEqual({
    token: 'einrichtungstoken',
    issuer: 'https://id.example.org',
    client_id: 'suchlage',
    client_secret: 'geheim',
  });
});

test('ohne Token im Link sagt die Seite warum und sendet nichts', async () => {
  renderWithApp(<SetupPage />, { route: '/einrichtung' });
  expect(screen.getByText(/Einrichtungslink ist unvollständig/)).toBeInTheDocument();
  expect(screen.getByRole('button', { name: 'Einrichten und anmelden' })).toBeDisabled();
});
