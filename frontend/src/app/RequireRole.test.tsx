import { screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { http, HttpResponse } from 'msw';
import { Route, Routes } from 'react-router';
import { meKiosk } from '../contract/fixtures.generated';
import { server } from '../test/server';
import { renderWithApp } from '../test/render';
import { RequireRole } from './RequireRole';

function Probe() {
  return (
    <Routes>
      <Route path="/anmelden" element={<p>Anmeldeseite</p>} />
      <Route
        path="/settings"
        element={
          <RequireRole roles={['admin']}>
            <p>Geheim</p>
          </RequireRole>
        }
      />
      <Route
        path="/"
        element={
          <RequireRole roles={['admin', 'operator', 'kiosk']}>
            <p>Lage</p>
          </RequireRole>
        }
      />
    </Routes>
  );
}

test('ohne Sitzung geht es zur Anmeldung', async () => {
  server.use(http.get('/api/me', () => HttpResponse.json({ error: 'Nicht angemeldet' }, { status: 401 })));
  renderWithApp(<Probe />, { route: '/' });
  expect(await screen.findByText('Anmeldeseite')).toBeInTheDocument();
});

test('falsche Rolle sieht einen Hinweis statt des Inhalts', async () => {
  renderWithApp(<Probe />, { route: '/settings' });
  expect(await screen.findByText('Diese Seite ist der Verwaltung vorbehalten.')).toBeInTheDocument();
  expect(screen.queryByText('Geheim')).not.toBeInTheDocument();
});

test('das Großbild bekommt einen eigenen Hinweis mit Weg zurück zur Lage', async () => {
  server.use(http.get('/api/me', () => HttpResponse.json(meKiosk)));
  renderWithApp(<Probe />, { route: '/settings' });
  expect(await screen.findByText('Das Großbild zeigt nur die Lage.')).toBeInTheDocument();
  expect(screen.getByRole('link', { name: 'Zur Lage' })).toHaveAttribute('href', '/');
});

test('passende Rolle sieht den Inhalt', async () => {
  renderWithApp(<Probe />, { route: '/' });
  expect(await screen.findByText('Lage')).toBeInTheDocument();
});

test('ist der Server weg, lässt sich die Prüfung erneut anstoßen', async () => {
  let up = false;
  server.use(
    http.get('/api/me', () =>
      up ? HttpResponse.json(meKiosk) : HttpResponse.json({ error: 'x' }, { status: 502 }),
    ),
  );
  renderWithApp(<Probe />, { route: '/' });
  expect(await screen.findByText(/Server nicht erreichbar/)).toBeInTheDocument();
  up = true;
  await userEvent.click(screen.getByRole('button', { name: 'Erneut versuchen' }));
  expect(await screen.findByText('Lage')).toBeInTheDocument();
});
