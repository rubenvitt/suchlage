import { act, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import {
  liveEvents,
  snapshotEmpty,
  snapshotFlying,
  snapshotGatewayOnly,
} from '../contract/fixtures.generated';
import type { LiveEvent } from '../contract/generated/LiveEvent';
import type { MapElementInput } from '../contract/generated/MapElementInput';
import type { MapElementUpdate } from '../contract/generated/MapElementUpdate';
import { http, HttpResponse } from 'msw';
import { ELEMENT_COLORS } from '../map/elementColors';
import type { SituationMap } from '../map/SituationMap';
import { server } from '../test/server';
import type { SituationSnapshot } from '../contract/generated/SituationSnapshot';
import { LiveProvider } from '../live/LiveProvider';
import { createLiveStore } from '../live/situationStore';
import { SyncCell } from '../shell/SyncCell';
import { renderWithApp } from '../test/render';
import { WorkstationView } from './WorkstationView';

// Die Karte selbst testet SituationMap.test.tsx; hier zählt, was die Ansicht ihr gibt und auf Klicks tut.
const karte = vi.hoisted(() => ({ props: null as Parameters<typeof SituationMap>[0] | null }));
vi.mock('../map/SituationMap', () => ({
  SituationMap: (props: Parameters<typeof SituationMap>[0]) => {
    karte.props = props;
    return <div data-testid="karte" />;
  },
}));
/** Klick in die Karte wie MapLibre ihn meldet. */
const klick = (lat: number, lon: number, elementId: string | null = null) =>
  act(() => karte.props!.onMapClick!({ lat, lon }, elementId));
vi.mock('../video/whep', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../video/whep')>()),
  startWhep: vi.fn(() => new Promise(() => {})),
}));

const eventOf = <T extends LiveEvent['type']>(type: T) =>
  liveEvents.find((e): e is Extract<LiveEvent, { type: T }> => e.type === type)!;

/**
 * Die Store-Uhr ist die echte Browseruhr: So rechnet `useServerNow` auf die Serverzeit des
 * Snapshots, und die Beispieltelemetrie ist frisch wie im Einsatz.
 */
function show(
  snapshot: SituationSnapshot | null,
  extra: LiveEvent[] = [],
  status: 'open' | 'reconnecting' = 'open',
) {
  const store = createLiveStore();
  if (snapshot) store.dispatch({ type: 'snapshot', snapshot });
  extra.forEach(store.dispatch);
  store.setStatus(status);
  return renderWithApp(
    <LiveProvider store={store}>
      <SyncCell />
      <WorkstationView />
    </LiveProvider>,
  );
}

test('vor dem ersten Stand wird die Lage abgerufen, nicht als leer gemeldet', async () => {
  show(null);
  expect(await screen.findByText('Lage wird abgerufen')).toBeInTheDocument();
  expect(screen.queryByText('Keine Fernsteuerung verbunden')).not.toBeInTheDocument();
  expect(screen.getByTestId('karte')).toBeInTheDocument();
});

test('ohne Fernsteuerung erklärt die Ansicht den nächsten Schritt', async () => {
  show(snapshotEmpty);
  expect(await screen.findByText('Keine Fernsteuerung verbunden')).toBeInTheDocument();
  expect(screen.getByText(/Einstellungen → Fernsteuerungen/)).toBeInTheDocument();
  expect(screen.getByTestId('karte')).toBeInTheDocument();
  expect(screen.queryByText('Am Boden')).not.toBeInTheDocument();
  expect(screen.queryByRole('button', { name: 'Livebild groß' })).not.toBeInTheDocument();
});

test('Fernsteuerung ohne Fluggerät', async () => {
  show(snapshotGatewayOnly);
  expect(await screen.findByText('Fluggerät aus')).toBeInTheDocument();
  expect(screen.getByText(/RC Plus 1 ist verbunden/)).toBeInTheDocument();
  expect(screen.queryByText('Am Boden')).not.toBeInTheDocument();
});

test('laufender Flug zeigt frische Leiste, Flugpaneel und Livebild', async () => {
  show(snapshotFlying);
  const strip = await screen.findByRole('region', { name: 'Telemetrie' });
  expect(strip).toHaveAttribute('data-freshness', 'fresh');
  const flight = screen.getByRole('region', { name: 'Flug läuft' });
  expect(within(flight).getByText('Flugzeit')).toBeInTheDocument();
  expect(within(flight).getByText('Strecke')).toBeInTheDocument();
  expect(within(flight).getByText('max. Höhe')).toBeInTheDocument();
  expect(screen.getByLabelText('Livebild der Drohne')).toBeInTheDocument();
  expect(screen.queryByText('Keine Fernsteuerung verbunden')).not.toBeInTheDocument();
  expect(screen.queryByText('Fluggerät aus')).not.toBeInTheDocument();
});

test('am Boden zeigt das Flugpaneel den letzten Flug', async () => {
  const aircraft = snapshotFlying.aircraft[0]!;
  show(snapshotFlying, [
    eventOf('flight_ended'),
    { type: 'aircraft', aircraft: { ...aircraft, phase: 'ground', flight_id: null } },
  ]);
  const panel = await screen.findByRole('region', { name: 'Am Boden' });
  expect(within(panel).getByText('Dauer')).toBeInTheDocument();
  expect(within(panel).getByText(/^beendet /)).toBeInTheDocument();
  expect(screen.queryByText('Flug läuft')).not.toBeInTheDocument();
});

test('Videofehler lässt Karte und Telemetrie weiterlaufen', async () => {
  show(snapshotFlying, [eventOf('stream')]);
  expect(await screen.findByText(/513011/)).toBeInTheDocument();
  expect(screen.getByRole('region', { name: 'Telemetrie' })).toHaveAttribute('data-freshness', 'fresh');
  expect(screen.getByTestId('karte')).toBeInTheDocument();
});

test('getrennter Live-Strom ist im Kopf sichtbar', async () => {
  show(snapshotFlying, [], 'reconnecting');
  expect(await screen.findByText('GETRENNT')).toBeInTheDocument();
});

test('Livebild groß tauscht die Spalten und lässt sich zurücknehmen', async () => {
  const { container } = show(snapshotFlying);
  const swap = await screen.findByRole('button', { name: 'Livebild groß' });
  expect(swap).toHaveAttribute('aria-pressed', 'false');
  await userEvent.click(swap);
  expect(swap).toHaveAttribute('aria-pressed', 'true');
  expect(container.querySelector('.sl-work')).toHaveAttribute('data-swap', 'true');
  await userEvent.click(swap);
  expect(container.querySelector('.sl-work')).toHaveAttribute('data-swap', 'false');
});

test('mit zwei Fernsteuerungen lässt sich die gezeigte wählen', async () => {
  const gateway = snapshotFlying.gateways[0]!;
  const spare = { ...gateway, sn: 'RCSIM0002', label: 'RC Plus Reserve', aircraft_sn: null };
  show({ ...snapshotFlying, gateways: [gateway, spare] });
  const choice = await screen.findByRole('radiogroup', { name: 'Fernsteuerung wählen' });
  await userEvent.click(within(choice).getByRole('radio', { name: 'RC Plus Reserve' }));
  expect(await screen.findByText('Fluggerät aus')).toBeInTheDocument();
  expect(screen.getByText(/RC Plus Reserve ist verbunden/)).toBeInTheDocument();
});

test('Operator zeichnet eine Fläche, benennt sie und legt sie an', async () => {
  let sent: MapElementInput | null = null;
  server.use(
    http.post('/api/map/elements', async ({ request }) => {
      sent = (await request.json()) as MapElementInput;
      const now = Date.now();
      return HttpResponse.json({
        ...sent,
        id: 'neu-1',
        points: sent.points.map((p) => ({ ...p, alt_m: null })),
        clamp_to_ground: true,
        author: 'Gruppenführer I&K',
        origin: 'web',
        created_at_ms: now,
        updated_at_ms: now,
      });
    }),
  );
  show(snapshotFlying);
  const tools = await screen.findByRole('toolbar', { name: 'Zeichnen' });
  await userEvent.click(within(tools).getByRole('button', { name: 'Fläche' }));
  const bar = screen.getByRole('toolbar', { name: 'Fläche zeichnen' });
  const fertig = within(bar).getByRole('button', { name: 'Fertig' });
  expect(karte.props!.follow).toBe(false);
  klick(52.43, 10.0);
  klick(52.44, 10.0);
  expect(fertig).toBeDisabled();
  klick(52.44, 10.01);
  expect(karte.props!.draft!.points).toHaveLength(3);
  await userEvent.click(fertig);

  const name = await screen.findByLabelText('Name');
  expect(name).toHaveValue('Fläche 1');
  await userEvent.clear(name);
  await userEvent.type(name, 'Abschnitt Nord');
  await userEvent.click(screen.getByRole('radio', { name: 'Gelb' }));
  await userEvent.click(screen.getByRole('button', { name: 'Anlegen' }));

  expect(await screen.findByRole('heading', { name: 'Abschnitt Nord' })).toBeInTheDocument();
  expect(sent).toEqual({
    name: 'Abschnitt Nord',
    kind: 'polygon',
    points: [
      { lat: 52.43, lon: 10.0 },
      { lat: 52.44, lon: 10.0 },
      { lat: 52.44, lon: 10.01 },
    ],
    color: ELEMENT_COLORS.find((c) => c.label === 'Gelb')!.wert,
  });
  // Die Antwort steht sofort in der Lage, ausgewählt; der Live-Strom bestätigt sie später.
  expect(karte.props!.elements!.map((e) => e.id)).toContain('neu-1');
  expect(karte.props!.selectedElementId).toBe('neu-1');
});

test('Escape bricht das Zeichnen ab, ein Punkt ist mit einem Klick fertig', async () => {
  show(snapshotFlying);
  await userEvent.click(await screen.findByRole('button', { name: 'Linie' }));
  klick(52.43, 10.0);
  await userEvent.keyboard('{Escape}');
  expect(karte.props!.draft).toBeNull();
  await userEvent.click(screen.getByRole('button', { name: 'Punkt' }));
  klick(52.43, 10.0);
  expect(await screen.findByRole('heading', { name: 'Neu: Punkt' })).toBeInTheDocument();
  await userEvent.click(screen.getByRole('button', { name: 'Verwerfen' }));
  expect(screen.queryByRole('heading', { name: 'Neu: Punkt' })).not.toBeInTheDocument();
});

test('ein Klick auf ein Element aus Pilot 2 öffnet es zum Umbenennen und Löschen', async () => {
  let renamed: MapElementUpdate | null = null;
  let deleted = '';
  const pin = eventOf('map_elements').elements[1]!;
  server.use(
    http.put('/api/map/elements/:id', async ({ request }) => {
      renamed = (await request.json()) as MapElementUpdate;
      return HttpResponse.json({ ...pin, ...renamed, updated_at_ms: pin.updated_at_ms + 1 });
    }),
    http.delete('/api/map/elements/:id', ({ params }) => {
      deleted = String(params.id);
      return new HttpResponse(null, { status: 204 });
    }),
  );
  show(snapshotFlying, [eventOf('map_elements')]);
  await screen.findByRole('toolbar', { name: 'Zeichnen' });
  klick(pin.points[0]!.lat, pin.points[0]!.lon, pin.id);
  expect(await screen.findByRole('heading', { name: 'Pilot 1' })).toBeInTheDocument();
  expect(screen.getByText(/aus Pilot 2 \(RC Plus 1\)/)).toBeInTheDocument();
  expect(screen.getByRole('radio', { name: 'Rot' })).toHaveAttribute('aria-checked', 'true');

  const name = screen.getByLabelText('Name');
  await userEvent.clear(name);
  await userEvent.type(name, 'Fundstelle');
  await userEvent.click(screen.getByRole('button', { name: 'Speichern' }));
  expect(await screen.findByRole('heading', { name: 'Fundstelle' })).toBeInTheDocument();
  expect(renamed).toEqual({ name: 'Fundstelle', color: pin.color });

  await userEvent.click(screen.getByRole('button', { name: 'Element löschen: Fundstelle' }));
  expect(await screen.findByText('„Fundstelle“ löschen?')).toBeInTheDocument();
  await userEvent.click(screen.getByRole('button', { name: 'Löschen' }));
  await vi.waitFor(() => expect(deleted).toBe(pin.id));
  await vi.waitFor(() =>
    expect(screen.queryByRole('heading', { name: 'Fundstelle' })).not.toBeInTheDocument(),
  );
  expect(karte.props!.elements!.map((e) => e.id)).not.toContain(pin.id);
});

test('ein Klick ins Leere schließt das gewählte Element', async () => {
  const pin = eventOf('map_elements').elements[1]!;
  show(snapshotFlying, [eventOf('map_elements')]);
  await screen.findByRole('toolbar', { name: 'Zeichnen' });
  klick(52.4, 10.0, pin.id);
  expect(await screen.findByRole('heading', { name: 'Pilot 1' })).toBeInTheDocument();
  klick(52.4, 10.0);
  expect(screen.queryByRole('heading', { name: 'Pilot 1' })).not.toBeInTheDocument();
});
