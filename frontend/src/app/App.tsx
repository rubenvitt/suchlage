import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { useState } from 'react';
import { RouterProvider } from 'react-router';
import { router } from './router';

export default function App() {
  const [client] = useState(
    () => new QueryClient({ defaultOptions: { queries: { retry: 1, refetchOnWindowFocus: false } } }),
  );
  const [browserRouter] = useState(router);
  return (
    <QueryClientProvider client={client}>
      <RouterProvider router={browserRouter} />
    </QueryClientProvider>
  );
}
