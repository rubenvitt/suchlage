import { render, screen } from '@testing-library/react';
import { http, HttpResponse } from 'msw';
import { ThemeModeProvider } from '../theme/ThemeModeProvider';
import { server } from '../test/server';
import App from './App';

test('ohne Sitzung landet der Einstieg auf der Anmeldung, nachts', async () => {
  server.use(http.get('/api/me', () => HttpResponse.json({ error: 'Nicht angemeldet' }, { status: 401 })));
  window.history.pushState({}, '', '/');
  render(
    <ThemeModeProvider>
      <App />
    </ThemeModeProvider>,
  );
  expect(await screen.findByRole('heading', { name: 'Suchlage' })).toBeInTheDocument();
  expect(window.location.pathname).toBe('/anmelden');
  expect(document.documentElement.dataset.theme).toBe('dark');
});
