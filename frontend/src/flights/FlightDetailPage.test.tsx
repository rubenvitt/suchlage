import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { http, HttpResponse } from 'msw';
import type { ReactNode } from 'react';
import { Route, Routes, useLocation } from 'react-router';
import { queryKeys } from '../api/queries';
import { flightDetail, meAdmin } from '../contract/fixtures.generated';
import { LiveProvider } from '../live/LiveProvider';
import { createLiveStore } from '../live/situationStore';
import { server } from '../test/server';
import { renderWithApp } from '../test/render';
import { FlightDetailPage } from './FlightDetailPage';

vi.mock('../map/SituationMap', () => ({
  SituationMap: (p: { tracks: unknown[][]; staticFit?: boolean; photos?: unknown[]; selectedPhotoId?: string | null }) => (
    <div
      data-testid="karte"
      data-segments={p.tracks[0]?.length}
      data-static={String(p.staticFit)}
      data-photos={p.photos?.length}
      data-selected={p.selectedPhotoId ?? ''}
    />
  ),
}));
vi.mock('../shell/Frame', () => ({ Frame: (p: { children: ReactNode }) => <>{p.children}</> }));

function Address() {
  const { pathname, search } = useLocation();
  return <output aria-label="Adresse">{pathname + search}</output>;
}

const page = (route = '/flights/6') =>
  renderWithApp(
    <LiveProvider store={createLiveStore()}>
      <Routes>
        <Route path="/flights/:id" element={<FlightDetailPage />} />
        <Route path="/flights" element={<p>Flugliste</p>} />
      </Routes>
      <Address />
    </LiveProvider>,
    { route },
  );

const asAdmin = () => server.use(http.get('/api/me', () => HttpResponse.json(meAdmin)));

test('Detail zeigt Spur, Kennzahlen und Video mit Download', async () => {
  page();
  expect(await screen.findByRole('link', { name: 'Video herunterladen' })).toHaveAttribute(
    'href',
    '/api/flights/6/video?download=1',
  );
  expect(screen.getByTestId('karte')).toHaveAttribute('data-segments', '2');
  expect(screen.getByTestId('karte')).toHaveAttribute('data-static', 'true');
  expect(screen.getByLabelText('Flugvideo')).toHaveAttribute('src', '/api/flights/6/video');
  expect(screen.getByText('18:00')).toBeInTheDocument();
  expect(screen.getByText('2,3')).toBeInTheDocument();
  expect(screen.getByText('gelandet')).toBeInTheDocument();
  expect(screen.getByText('RCSIM0001')).toBeInTheDocument();
  expect(screen.getByRole('link', { name: 'Alle Flüge' })).toHaveAttribute('href', '/flights');
});

test('ohne Aufnahme sagt das Detail, warum', async () => {
  server.use(http.get('/api/flights/:id', () => HttpResponse.json({ ...flightDetail, video: null })));
  page();
  expect(await screen.findByText(/Kein Video aufgezeichnet/)).toBeInTheDocument();
  expect(screen.queryByLabelText('Flugvideo')).not.toBeInTheDocument();
});

test('bei laufendem Flug kommt das Video nach der Landung', async () => {
  const flight = { ...flightDetail.flight, ended_at_ms: null, end_reason: null };
  server.use(http.get('/api/flights/:id', () => HttpResponse.json({ ...flightDetail, flight, video: null })));
  page();
  expect(await screen.findByText(/das Video gibt es nach der Landung/)).toBeInTheDocument();
  expect(screen.getByText('läuft')).toBeInTheDocument();
});

test('ein unbekannter Flug führt zurück zur Liste', async () => {
  server.use(http.get('/api/flights/:id', () => HttpResponse.json({ error: 'x' }, { status: 404 })));
  page('/flights/99');
  expect(await screen.findByRole('alert')).toHaveTextContent('Flug 99 gibt es nicht.');
  expect(screen.getByRole('link', { name: 'Zur Flugliste' })).toHaveAttribute('href', '/flights');
});

test('eine Adresse ohne Flugnummer fragt den Server nicht', async () => {
  const asked = vi.fn();
  server.use(http.get('/api/flights/:id', () => (asked(), HttpResponse.json(flightDetail))));
  page('/flights/abc');
  expect(await screen.findByRole('alert')).toHaveTextContent('Flug abc gibt es nicht.');
  expect(asked).not.toHaveBeenCalled();
});

test('Fehler beim Abruf zeigt den Fehlerzustand mit erneutem Abruf', async () => {
  server.use(http.get('/api/flights/:id', () => HttpResponse.json({ error: 'x' }, { status: 500 })));
  page();
  expect(await screen.findByText('Daten nicht abrufbar')).toBeInTheDocument();
  expect(screen.getByRole('button', { name: 'Erneut abrufen' })).toBeInTheDocument();
});

test('Fotos des Flugs: Leiste mit Vorschaubildern, ein Klick wählt die Auslösung und öffnet die Vorschau', async () => {
  page();
  const strip = await screen.findByRole('button', { name: /DJI_20261017143012_0001_T\.JPG/ });
  expect(screen.getByTestId('karte')).toHaveAttribute('data-photos', '2');
  expect(screen.getByText('2 Auslösungen')).toBeInTheDocument();
  await userEvent.click(strip);
  expect(strip).toHaveAttribute('aria-pressed', 'true');
  expect(screen.getByTestId('karte')).toHaveAttribute('data-selected', 'RCSIM0001/DJI_20261017143012_0001');
  expect(screen.getByRole('region', { name: /^Foto / })).toBeInTheDocument();
});

test('ohne Fotos sagt das Detail, woher sie kämen', async () => {
  server.use(http.get('/api/flights/:id/photos', () => HttpResponse.json([])));
  page();
  expect(await screen.findByText(/Keine Fotos/)).toBeInTheDocument();
});

test('Bedienung sieht keine Löschknöpfe', async () => {
  page();
  await userEvent.click(await screen.findByRole('button', { name: /DJI_20261017143012_0001_T\.JPG/ }));
  expect(screen.getByRole('region', { name: /^Foto / })).toBeInTheDocument();
  for (const name of ['Flug löschen …', 'Video löschen …', 'Foto löschen …'])
    expect(screen.queryByRole('button', { name })).not.toBeInTheDocument();
});

test('„Flug löschen …“ fragt nach, nennt was mitgeht, und führt danach zur Flugliste', async () => {
  asAdmin();
  const deleted = vi.fn();
  server.use(http.delete('/api/flights/:id', ({ params }) => (deleted(params.id), new HttpResponse(null, { status: 204 }))));
  page();
  await userEvent.click(await screen.findByRole('button', { name: 'Flug löschen …' }));
  const dialog = await screen.findByRole('dialog', { name: 'Flug 6 löschen?' });
  expect(dialog).toHaveTextContent('Spur, Video und 2 Auslösungen');
  expect(deleted).not.toHaveBeenCalled();
  await userEvent.click(within(dialog).getByRole('button', { name: 'Endgültig löschen' }));
  expect(await screen.findByText('Flugliste')).toBeInTheDocument();
  expect(deleted).toHaveBeenCalledWith('6');
  expect(screen.getByLabelText('Adresse')).toHaveTextContent(/^\/flights$/);
});

test('Flug löschen zeigt den Text des Servers', async () => {
  asAdmin();
  server.use(
    http.delete('/api/flights/:id', () =>
      HttpResponse.json({ error: 'Ein laufender Flug lässt sich nicht löschen' }, { status: 422 }),
    ),
  );
  page();
  await userEvent.click(await screen.findByRole('button', { name: 'Flug löschen …' }));
  await userEvent.click(await screen.findByRole('button', { name: 'Endgültig löschen' }));
  expect(await screen.findByText('Ein laufender Flug lässt sich nicht löschen')).toBeInTheDocument();
  expect(screen.getByLabelText('Adresse')).toHaveTextContent('/flights/6');
});

test('„Video löschen …“ löscht nur die Aufzeichnung und zeigt Fehler des Servers', async () => {
  asAdmin();
  const deleted = vi.fn();
  server.use(
    http.delete('/api/flights/:id/video', () => {
      deleted();
      return HttpResponse.json({ error: 'Videoablage nicht eingehängt' }, { status: 503 });
    }),
  );
  page();
  await userEvent.click(await screen.findByRole('button', { name: 'Video löschen …' }));
  const dialog = await screen.findByRole('dialog', { name: 'Video von Flug 6 löschen?' });
  await userEvent.click(within(dialog).getByRole('button', { name: 'Endgültig löschen' }));
  expect(await within(dialog).findByText('Videoablage nicht eingehängt')).toBeInTheDocument();
  expect(deleted).toHaveBeenCalledTimes(1);
});

test('ohne Video kein „Video löschen …“', async () => {
  asAdmin();
  server.use(http.get('/api/flights/:id', () => HttpResponse.json({ ...flightDetail, video: null })));
  page();
  expect(await screen.findByRole('button', { name: 'Flug löschen …' })).toBeInTheDocument();
  expect(screen.queryByRole('button', { name: 'Video löschen …' })).not.toBeInTheDocument();
});

test('„Foto löschen …“ löscht die Auslösung mit allen Linsen', async () => {
  asAdmin();
  const deleted = vi.fn();
  server.use(
    http.delete('/api/photos/shots/:shot', ({ request }) => (deleted(new URL(request.url).pathname), new HttpResponse(null, { status: 204 }))),
  );
  page();
  await userEvent.click(await screen.findByRole('button', { name: /DJI_20261017143012_0001_T\.JPG/ }));
  await userEvent.click(screen.getByRole('button', { name: 'Foto löschen …' }));
  const dialog = await screen.findByRole('dialog', { name: /^Foto .* löschen\?$/ });
  expect(dialog).toHaveTextContent('alle 3 Linsen');
  await userEvent.click(within(dialog).getByRole('button', { name: 'Endgültig löschen' }));
  await waitFor(() =>
    expect(deleted).toHaveBeenCalledWith('/api/photos/shots/RCSIM0001%2FDJI_20261017143012_0001'),
  );
});

test('ein anderswo gelöschter Flug führt beim Neuladen zur Flugliste statt zu einem Fehler', async () => {
  const { client } = page();
  expect(await screen.findByRole('link', { name: 'Video herunterladen' })).toBeInTheDocument();
  server.use(http.get('/api/flights/:id', () => HttpResponse.json({ error: 'x' }, { status: 404 })));
  await client.invalidateQueries({ queryKey: queryKeys.flight(6) });
  expect(await screen.findByText('Flugliste')).toBeInTheDocument();
  expect(screen.queryByRole('alert')).not.toBeInTheDocument();
});
