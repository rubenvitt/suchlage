import { screen } from '@testing-library/react';
import { http, HttpResponse } from 'msw';
import userEvent from '@testing-library/user-event';
import { Link, Route, Routes } from 'react-router';
import { RequireRole } from '../app/RequireRole';
import { meKiosk } from '../contract/fixtures.generated';
import { server } from '../test/server';
import { renderWithApp } from '../test/render';
import KioskEnrollPage from './KioskEnrollPage';

afterEach(() => {
  window.location.hash = '';
});

test('widerrufener Zugangslink zeigt die Server-Meldung', async () => {
  window.location.hash = '#widerrufen';
  server.use(
    http.post('/api/auth/kiosk', () =>
      HttpResponse.json({ error: 'Zugangslink ungültig oder widerrufen' }, { status: 401 }),
    ),
  );
  renderWithApp(<KioskEnrollPage />, { route: '/kiosk' });
  expect(await screen.findByText('Zugangslink ungültig oder widerrufen')).toBeInTheDocument();
});

test('ohne Token im Link erklärt die Seite, was zu tun ist, und fragt den Server nicht', async () => {
  let asked = false;
  server.use(
    http.post('/api/auth/kiosk', () => {
      asked = true;
      return new HttpResponse(null, { status: 204 });
    }),
  );
  renderWithApp(<KioskEnrollPage />, { route: '/kiosk' });
  expect(screen.getByRole('alert')).toHaveTextContent('Der Zugangslink ist unvollständig.');
  await new Promise((resolve) => setTimeout(resolve, 50));
  expect(asked).toBe(false);
});

test('gültiger Zugangslink meldet das Gerät an und führt zur Lage, ohne Token in der Adresse', async () => {
  window.location.hash = '#gueltig';
  let token: unknown;
  server.use(
    http.post('/api/auth/kiosk', async ({ request }) => {
      token = ((await request.json()) as { token: string }).token;
      return new HttpResponse(null, { status: 204 });
    }),
  );
  renderWithApp(
    <Routes>
      <Route path="/kiosk" element={<KioskEnrollPage />} />
      <Route path="/" element={<p>Lage</p>} />
    </Routes>,
    { route: '/kiosk' },
  );
  expect(await screen.findByText('Lage')).toBeInTheDocument();
  expect(token).toBe('gueltig');
  expect(window.location.hash).toBe('');
});

test('war das Gerät vorher abgemeldet, führt die Kopplung trotzdem zur Lage', async () => {
  window.location.hash = '#gueltig';
  let enrolled = false;
  server.use(
    http.get('/api/me', () =>
      enrolled
        ? HttpResponse.json(meKiosk)
        : HttpResponse.json({ error: 'Nicht angemeldet' }, { status: 401 }),
    ),
    http.post('/api/auth/kiosk', () => {
      enrolled = true;
      return new HttpResponse(null, { status: 204 });
    }),
  );
  renderWithApp(
    <Routes>
      <Route path="/kiosk" element={<KioskEnrollPage />} />
      <Route path="/anmelden" element={<Link to="/kiosk">Koppeln</Link>} />
      <Route
        path="/"
        element={
          <RequireRole roles={['kiosk']}>
            <p>Lage</p>
          </RequireRole>
        }
      />
    </Routes>,
    { route: '/' },
  );
  // Erst die Lage ohne Sitzung (`me` ist dann `null` im Cache), danach der Zugangslink.
  await userEvent.click(await screen.findByRole('link', { name: 'Koppeln' }));
  expect(await screen.findByText('Lage')).toBeInTheDocument();
});
