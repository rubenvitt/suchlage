import { screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { http, HttpResponse } from 'msw';
import { Route, Routes } from 'react-router';
import { meAdmin } from '../contract/fixtures.generated';
import { server } from '../test/server';
import { renderWithApp } from '../test/render';
import { UserMenu } from './UserMenu';

test('Abmelden beendet die Sitzung und führt zur Anmeldung', async () => {
  let loggedOut = false;
  server.use(
    http.post('/api/auth/logout', () => {
      loggedOut = true;
      return new HttpResponse(null, { status: 204 });
    }),
  );
  renderWithApp(
    <Routes>
      <Route path="/" element={<UserMenu me={meAdmin} />} />
      <Route path="/anmelden" element={<p>Anmeldeseite</p>} />
    </Routes>,
  );
  await userEvent.click(screen.getByRole('button', { name: 'Benutzermenü von Erika Mustermann' }));
  await userEvent.click(await screen.findByText('Abmelden'));
  expect(await screen.findByText('Anmeldeseite')).toBeInTheDocument();
  expect(loggedOut).toBe(true);
});

test('Darstellung lässt sich auf Hell stellen', async () => {
  renderWithApp(<UserMenu me={meAdmin} />);
  await userEvent.click(screen.getByRole('button', { name: 'Benutzermenü von Erika Mustermann' }));
  await userEvent.click(await screen.findByText('Hell'));
  expect(document.documentElement.dataset.theme).toBe('light');
});
