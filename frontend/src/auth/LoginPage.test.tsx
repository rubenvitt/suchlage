import { screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { http, HttpResponse } from 'msw';
import { Route, Routes } from 'react-router';
import { RequireRole } from '../app/RequireRole';
import { authOptionsDev, authOptionsSetup, meOperator } from '../contract/fixtures.generated';
import { server } from '../test/server';
import { renderWithApp } from '../test/render';
import LoginPage from './LoginPage';

test('Pocket-ID-Anmeldung führt zum Start mit Rücksprungziel', async () => {
  renderWithApp(<LoginPage />, { route: '/anmelden?next=/flights' });
  const link = await screen.findByRole('link', { name: 'Mit Pocket ID anmelden' });
  expect(link).toHaveAttribute('href', '/api/auth/oidc/start?next=%2Fflights');
});

test('ein Rücksprungziel auf eine fremde Adresse wird verworfen', async () => {
  renderWithApp(<LoginPage />, { route: '/anmelden?next=//boese.example.org/x' });
  const link = await screen.findByRole('link', { name: 'Mit Pocket ID anmelden' });
  expect(link).toHaveAttribute('href', '/api/auth/oidc/start?next=%2F');
});

test('im Dev-Modus gibt es die Entwicklungsanmeldung', async () => {
  server.use(http.get('/api/auth/options', () => HttpResponse.json(authOptionsDev)));
  let called = false;
  server.use(
    http.post('/api/auth/dev-login', () => {
      called = true;
      return new HttpResponse(null, { status: 204 });
    }),
  );
  renderWithApp(<LoginPage />, { route: '/anmelden' });
  await userEvent.click(await screen.findByRole('button', { name: 'Als Entwicklung anmelden' }));
  expect(called).toBe(true);
});

test('nach der Entwicklungsanmeldung führt ein Klick zur Lage, nicht zurück zur Anmeldung', async () => {
  // Die Lage hat `me` schon als `null` (401) im Cache; die Anmeldung muss es neu holen, bevor sie weiterleitet.
  let signedIn = false;
  server.use(
    http.get('/api/auth/options', () => HttpResponse.json(authOptionsDev)),
    http.get('/api/me', () =>
      signedIn
        ? HttpResponse.json(meOperator)
        : HttpResponse.json({ error: 'Nicht angemeldet' }, { status: 401 }),
    ),
    http.post('/api/auth/dev-login', () => {
      signedIn = true;
      return new HttpResponse(null, { status: 204 });
    }),
  );
  renderWithApp(
    <Routes>
      <Route path="/anmelden" element={<LoginPage />} />
      <Route
        path="/"
        element={
          <RequireRole roles={['operator']}>
            <p>Lage</p>
          </RequireRole>
        }
      />
    </Routes>,
    { route: '/' },
  );
  await userEvent.click(await screen.findByRole('button', { name: 'Als Entwicklung anmelden' }));
  expect(await screen.findByText('Lage')).toBeInTheDocument();
});

test('scheitert die Entwicklungsanmeldung, steht die Server-Meldung da', async () => {
  server.use(
    http.get('/api/auth/options', () => HttpResponse.json(authOptionsDev)),
    http.post('/api/auth/dev-login', () =>
      HttpResponse.json({ error: 'Datenbank nicht erreichbar' }, { status: 503 }),
    ),
  );
  renderWithApp(<LoginPage />, { route: '/anmelden' });
  await userEvent.click(await screen.findByRole('button', { name: 'Als Entwicklung anmelden' }));
  expect(await screen.findByText('Datenbank nicht erreichbar')).toBeInTheDocument();
});

test('nicht eingerichtet: Hinweis auf den Einrichtungslink', async () => {
  server.use(http.get('/api/auth/options', () => HttpResponse.json(authOptionsSetup)));
  renderWithApp(<LoginPage />, { route: '/anmelden' });
  expect(await screen.findByText(/Einrichtungslink aus dem Server-Protokoll/)).toBeInTheDocument();
});

test('abgebrochene Anmeldung wird erklärt', async () => {
  renderWithApp(<LoginPage />, { route: '/anmelden?fehler=oidc' });
  expect(await screen.findByText('Anmeldung abgebrochen. Bitte erneut anmelden.')).toBeInTheDocument();
});

test('während des Ladens und bei Serverfehler steht eine Aussage da, kein leeres Feld', async () => {
  server.use(http.get('/api/auth/options', () => HttpResponse.json({ error: 'x' }, { status: 500 })));
  renderWithApp(<LoginPage />, { route: '/anmelden' });
  expect(screen.getByText('Anmeldeverfahren werden abgerufen …')).toBeInTheDocument();
  expect(await screen.findByText(/Server nicht erreichbar/)).toBeInTheDocument();
  expect(screen.getByRole('button', { name: 'Erneut versuchen' })).toBeInTheDocument();
});
