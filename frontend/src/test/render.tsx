import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render } from '@testing-library/react';
import type { ReactElement, ReactNode } from 'react';
import { MemoryRouter } from 'react-router';
import { ThemeModeProvider } from '../theme/ThemeModeProvider';

/**
 * Rendert wie die App: Abfragen, Darstellung, Router. Die Hülle geht über `wrapper`, damit sie auch
 * `rerender` übersteht. Eine andere Rolle setzt der Test per `server.use(…/api/me…)`. `client` gibt
 * Tests den Zugriff auf die Abfragen, etwa für einen Hintergrund-Abruf.
 */
export function renderWithApp(ui: ReactElement, { route = '/' }: { route?: string } = {}) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  function Wrapper({ children }: { children: ReactNode }) {
    return (
      <QueryClientProvider client={client}>
        <ThemeModeProvider>
          <MemoryRouter initialEntries={[route]}>{children}</MemoryRouter>
        </ThemeModeProvider>
      </QueryClientProvider>
    );
  }
  return { ...render(ui, { wrapper: Wrapper }), client };
}
