// Ersatz für Lifeline Hub frontend/src/test/utils.tsx: nur `renderMitProviders`, ohne Lifelines Auth und Query-Fabrik.
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, type RenderOptions } from '@testing-library/react';
import { App as AntApp, ConfigProvider } from 'antd';
import type { ReactElement, ReactNode } from 'react';
import { MemoryRouter } from 'react-router';

export function renderMitProviders(ui: ReactElement, options: Omit<RenderOptions, 'wrapper'> & { route?: string } = {}) {
  const { route = '/', ...rest } = options;
  const client = new QueryClient({ defaultOptions: { queries: { retry: false, gcTime: 0 } } });
  function Wrapper({ children }: { children: ReactNode }) {
    return (
      <QueryClientProvider client={client}>
        <ConfigProvider>
          <AntApp>
            <MemoryRouter initialEntries={[route]}>{children}</MemoryRouter>
          </AntApp>
        </ConfigProvider>
      </QueryClientProvider>
    );
  }
  return { client, ...render(ui, { wrapper: Wrapper, ...rest }) };
}
