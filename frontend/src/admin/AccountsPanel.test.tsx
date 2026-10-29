import { screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { http, HttpResponse } from 'msw';
import { server } from '../test/server';
import { renderWithApp } from '../test/render';
import { AccountsPanel } from './AccountsPanel';

test('ein Konto wird nach Rückfrage gesperrt, ein gesperrtes entsperrt, das eigene nie', async () => {
  const calls: string[] = [];
  server.use(
    http.put('/api/accounts/:id/block', ({ params }) => {
      calls.push(`sperren ${String(params.id)}`);
      return new HttpResponse(null, { status: 204 });
    }),
    http.delete('/api/accounts/:id/block', ({ params }) => {
      calls.push(`entsperren ${String(params.id)}`);
      return new HttpResponse(null, { status: 204 });
    }),
  );
  renderWithApp(<AccountsPanel />);
  expect(await screen.findByText('Max Muster')).toBeInTheDocument();
  expect(screen.getByText('(eigenes Konto)')).toBeInTheDocument();
  expect(screen.queryByRole('button', { name: /Erika Mustermann/ })).not.toBeInTheDocument();
  expect(screen.getByText(/^gesperrt seit/)).toBeInTheDocument();

  await userEvent.click(screen.getByRole('button', { name: 'Konto sperren: Max Muster' }));
  expect(await screen.findByText('Konto „Max Muster“ sperren?')).toBeInTheDocument();
  await userEvent.click(screen.getByRole('button', { name: 'Sperren' }));
  await vi.waitFor(() => expect(calls).toEqual(['sperren 2']));

  await userEvent.click(screen.getByRole('button', { name: 'Sperre aufheben: Gesa Gast' }));
  await vi.waitFor(() => expect(calls).toEqual(['sperren 2', 'entsperren 3']));
});

test('Fehler des Servers steht unter der Liste', async () => {
  server.use(
    http.put('/api/accounts/:id/block', () =>
      HttpResponse.json({ error: 'Das eigene Konto lässt sich nicht sperren' }, { status: 422 }),
    ),
  );
  renderWithApp(<AccountsPanel />);
  await userEvent.click(await screen.findByRole('button', { name: 'Konto sperren: Max Muster' }));
  await userEvent.click(await screen.findByRole('button', { name: 'Sperren' }));
  expect(await screen.findByText('Das eigene Konto lässt sich nicht sperren')).toBeInTheDocument();
});
