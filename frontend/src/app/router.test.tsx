import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen } from '@testing-library/react';
import { createMemoryRouter, RouterProvider } from 'react-router';
import { ThemeModeProvider } from '../theme/ThemeModeProvider';
import { routes } from './router';

vi.mock('../views/LagePage', () => ({
  LagePage: () => {
    throw new Error('Absturz in der Lage');
  },
}));

beforeEach(() => {
  // Die Session-Routen öffnen den Live-Strom; jsdom kennt keine EventSource.
  vi.stubGlobal(
    'EventSource',
    class {
      close() {}
    },
  );
});

afterEach(() => vi.unstubAllGlobals());

function renderAt(path: string) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  render(
    <QueryClientProvider client={client}>
      <ThemeModeProvider>
        <RouterProvider router={createMemoryRouter(routes, { initialEntries: [path] })} />
      </ThemeModeProvider>
    </QueryClientProvider>,
  );
}

test('eine unbekannte Adresse zeigt einen Hinweis mit Weg zur Lage', async () => {
  renderAt('/gibtsnicht');
  expect(await screen.findByText('Diese Seite gibt es nicht.')).toBeInTheDocument();
  expect(screen.getByRole('link', { name: 'Zur Lage' })).toHaveAttribute('href', '/');
});

test('stürzt eine Seite ab, steht ein deutscher Hinweis da, der sich selbst erneuert', async () => {
  vi.spyOn(console, 'error').mockImplementation(() => {}); // React meldet den gefangenen Fehler
  renderAt('/');
  expect(await screen.findByRole('heading', { name: 'Die Ansicht ist abgestürzt.' })).toBeInTheDocument();
  expect(screen.getByText(/Neuer Versuch in 30 s/)).toBeInTheDocument();
  expect(screen.getByRole('button', { name: 'Neu laden' })).toBeInTheDocument();
});
