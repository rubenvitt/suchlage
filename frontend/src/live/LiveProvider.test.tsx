import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { act, render, screen } from '@testing-library/react';
import { http, HttpResponse } from 'msw';
import type { ReactNode } from 'react';
import { useFlight, useFlights, useMapPhotos, useMissions } from '../api/queries';
import { flightDetail, flightList, liveEvents, snapshotFlying } from '../contract/fixtures.generated';
import type { LiveEvent } from '../contract/generated/LiveEvent';
import { server } from '../test/server';
import { LiveProvider, useConnectionStatus, useLive, useServerNow } from './LiveProvider';
import { createLiveStore } from './situationStore';

class FakeSource {
  static last: FakeSource;
  onopen: (() => void) | null = null;
  onmessage: ((e: { data: string }) => void) | null = null;
  onerror: (() => void) | null = null;
  readyState = 0;
  constructor(public url: string) {
    FakeSource.last = this;
  }
  close() {}
}

const send = (e: LiveEvent) => act(() => FakeSource.last.onmessage?.({ data: JSON.stringify(e) }));

function withClient(children: ReactNode) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return <QueryClientProvider client={client}>{children}</QueryClientProvider>;
}

function Probe() {
  const status = useConnectionStatus();
  const count = useLive((s) => Object.keys(s.aircraft).length);
  const flights = useFlights({ kind: 'none' });
  return (
    <p>
      {status} · {count} Fluggeräte · {flights.data?.length ?? '–'} Flüge
    </p>
  );
}

afterEach(() => {
  vi.unstubAllGlobals();
  vi.useRealTimers();
});

/** Zählt die Abrufe von Flugliste und Flugdetail. */
function countFlightRequests() {
  const count = { list: 0, detail: 0 };
  server.use(
    http.get('/api/flights', () => {
      count.list += 1;
      return HttpResponse.json(flightList);
    }),
    http.get('/api/flights/:id', () => {
      count.detail += 1;
      return HttpResponse.json(flightDetail);
    }),
  );
  return count;
}

function DetailProbe() {
  const detail = useFlight(flightDetail.flight.id);
  return <p>{detail.data ? 'Detail da' : 'Detail lädt'}</p>;
}

test('verbindet sich selbst, zeigt Live-Stand und erneuert die Flugliste bei Flugereignissen', async () => {
  vi.stubGlobal('EventSource', FakeSource);
  const count = countFlightRequests();
  render(
    withClient(
      <LiveProvider url="/api/live">
        <Probe />
      </LiveProvider>,
    ),
  );
  expect(FakeSource.last.url).toBe('/api/live');
  expect(
    await screen.findByText(`connecting · 0 Fluggeräte · ${flightList.length} Flüge`),
  ).toBeInTheDocument();
  expect(count.list).toBe(1);

  act(() => FakeSource.last.onopen?.());
  send({ type: 'snapshot', snapshot: snapshotFlying });
  expect(screen.getByText(`open · 1 Fluggeräte · ${flightList.length} Flüge`)).toBeInTheDocument();
  await vi.waitFor(() => expect(count.list).toBe(2));

  send(liveEvents.find((e) => e.type === 'aircraft')!);
  await new Promise((r) => setTimeout(r, 20));
  expect(count.list).toBe(2);
  send(liveEvents.find((e) => e.type === 'flight_ended')!);
  await vi.waitFor(() => expect(count.list).toBe(3));
});

// Fällt eine Landung in eine SSE-Lücke, kommt beim Neuverbinden nur ein Snapshot.
test('nach dem Neuverbinden laden Flugliste und Flugdetail neu', async () => {
  vi.stubGlobal('EventSource', FakeSource);
  const count = countFlightRequests();
  render(
    withClient(
      <LiveProvider url="/api/live">
        <Probe />
        <DetailProbe />
      </LiveProvider>,
    ),
  );
  expect(await screen.findByText('Detail da')).toBeInTheDocument();
  await vi.waitFor(() => expect(count).toEqual({ list: 1, detail: 1 }));

  act(() => FakeSource.last.onerror?.());
  send({ type: 'snapshot', snapshot: snapshotFlying });
  await vi.waitFor(() => expect(count).toEqual({ list: 2, detail: 2 }));
});

function MissionsProbe() {
  const missions = useMissions();
  const photos = useMapPhotos({ kind: 'none' });
  return <p>{missions.data && photos.data ? 'Einsätze da' : 'Einsätze laden'}</p>;
}

// Ein Einsatz, der in einer SSE-Lücke startet oder schließt, fehlte sonst bis zum Neuladen der Seite.
test('nach dem Neuverbinden laden die Einsätze neu, eine Einsatzänderung auch die Fotos', async () => {
  vi.stubGlobal('EventSource', FakeSource);
  const count = { missions: 0, photos: 0 };
  server.use(
    http.get('/api/missions', () => {
      count.missions += 1;
      return HttpResponse.json([]);
    }),
    http.get('/api/photos', () => {
      count.photos += 1;
      return HttpResponse.json([]);
    }),
  );
  render(
    withClient(
      <LiveProvider url="/api/live">
        <MissionsProbe />
      </LiveProvider>,
    ),
  );
  expect(await screen.findByText('Einsätze da')).toBeInTheDocument();
  expect(count).toEqual({ missions: 1, photos: 1 });

  act(() => FakeSource.last.onerror?.());
  send({ type: 'snapshot', snapshot: snapshotFlying });
  await vi.waitFor(() => expect(count).toEqual({ missions: 2, photos: 2 }));

  send({ type: 'missions_changed' });
  await vi.waitFor(() => expect(count).toEqual({ missions: 3, photos: 3 }));
});

test('ein übergebener Speicher ersetzt die Verbindung', () => {
  const create = vi.fn();
  vi.stubGlobal('EventSource', create);
  const store = createLiveStore(() => snapshotFlying.server_time_ms);
  store.dispatch({ type: 'snapshot', snapshot: snapshotFlying });
  render(
    withClient(
      <LiveProvider store={store}>
        <Probe />
      </LiveProvider>,
    ),
  );
  expect(screen.getByText(/1 Fluggeräte/)).toBeInTheDocument();
  expect(create).not.toHaveBeenCalled();
});

test('Serverzeit läuft sekündlich mit dem gemessenen Versatz', () => {
  vi.useFakeTimers({ now: 1_000_000 });
  const store = createLiveStore(() => snapshotFlying.server_time_ms - 2_000);
  store.dispatch({ type: 'snapshot', snapshot: snapshotFlying });
  function Now() {
    return <p>{useServerNow()}</p>;
  }
  render(
    withClient(
      <LiveProvider store={store}>
        <Now />
      </LiveProvider>,
    ),
  );
  expect(screen.getByText(String(1_002_000))).toBeInTheDocument();
  act(() => vi.advanceTimersByTime(1_000));
  expect(screen.getByText(String(1_003_000))).toBeInTheDocument();
});

test('ohne Provider ein klarer Fehler', () => {
  vi.spyOn(console, 'error').mockImplementation(() => {});
  expect(() => render(<Probe />)).toThrow('useLive außerhalb von <LiveProvider>');
});
