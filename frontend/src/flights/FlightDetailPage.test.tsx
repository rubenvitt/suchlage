import { screen } from '@testing-library/react';
import { http, HttpResponse } from 'msw';
import type { ReactNode } from 'react';
import { Route, Routes } from 'react-router';
import { flightDetail } from '../contract/fixtures.generated';
import { LiveProvider } from '../live/LiveProvider';
import { createLiveStore } from '../live/situationStore';
import { server } from '../test/server';
import { renderWithApp } from '../test/render';
import { FlightDetailPage } from './FlightDetailPage';

vi.mock('../map/SituationMap', () => ({
  SituationMap: (p: { tracks: unknown[][]; staticFit?: boolean }) => (
    <div data-testid="karte" data-segments={p.tracks[0]?.length} data-static={String(p.staticFit)} />
  ),
}));
vi.mock('../shell/Frame', () => ({ Frame: (p: { children: ReactNode }) => <>{p.children}</> }));

const page = (route = '/flights/6') =>
  renderWithApp(
    <LiveProvider store={createLiveStore()}>
      <Routes>
        <Route path="/flights/:id" element={<FlightDetailPage />} />
      </Routes>
    </LiveProvider>,
    { route },
  );

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
