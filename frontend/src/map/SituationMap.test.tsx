import { act, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { ConfigProvider, theme } from 'antd';
import type { ReactElement } from 'react';
import { snapshotFlying } from '../contract/fixtures.generated';
import type { Freshness } from '../live/freshness';
import { farbenHell } from '../theme/tokens';
import { MAP_UNAVAILABLE_TEXT, MAP_UNAVAILABLE_TITLE } from './MapBoundary';
import { SituationMap } from './SituationMap';

// jsdom hat kein WebGL: Die Karte selbst ersetzt ein Double, das festhält, was die Komponente ihr sagt.
const { FakeMap, GPUInitializationError, maps, gpu } = vi.hoisted(() => {
  const maps: InstanceType<typeof FakeMap>[] = [];
  /** Wie MapLibre ohne WebGL 2: Der Konstruktor wirft, solange `available` falsch ist. */
  const gpu = { available: true, fault: null as Error | null };
  class GPUInitializationError extends Error {}
  class FakeMap {
    handlers = new Map<string, (e: object) => void>();
    sources = new Map<string, { setData: ReturnType<typeof vi.fn> }>();
    center: unknown;
    zoom: number;
    touchZoomRotate = { disableRotation: vi.fn() };
    keyboard = { disableRotation: vi.fn() };
    addImage = vi.fn();
    addLayer = vi.fn();
    fitBounds = vi.fn();
    easeTo = vi.fn();
    zoomIn = vi.fn();
    remove = vi.fn();
    constructor(public options: { container: unknown; center: unknown; zoom: number }) {
      // Reihenfolge wie in MapLibre: erst der Behälter, dann der Grafikkontext.
      if (!options.container) throw new Error('Container not found');
      if (!gpu.available) throw new GPUInitializationError('webgl2');
      if (gpu.fault) throw gpu.fault;
      this.center = options.center;
      this.zoom = options.zoom;
      maps.push(this);
    }
    on(event: string, handler: (e: object) => void) {
      this.handlers.set(event, handler);
    }
    hasImage = () => false;
    addSource(id: string) {
      this.sources.set(id, { setData: vi.fn() });
    }
    getSource(id: string) {
      return this.sources.get(id);
    }
    getCenter = () => this.center;
    getZoom = () => this.zoom;
    /** Zuletzt gesetzte Features einer Quelle. */
    features(id: string) {
      return this.sources.get(id)!.setData.mock.calls.at(-1)![0].features;
    }
  }
  return { FakeMap, GPUInitializationError, maps, gpu };
});
vi.mock('maplibre-gl', () => ({ Map: FakeMap, GPUInitializationError }));
vi.mock('./maplibreWorker', () => ({}));
vi.mock('./glyphs', () => ({
  aircraftGlyph: (color: string, outlineOnly: boolean) => ({ color, outlineOnly }),
  homeGlyph: () => ({}),
}));
vi.mock('./unitSign', () => ({ registerUnitSign: () => {}, UNIT_IMAGE: 'unit-drone-team' }));

const s = snapshotFlying;
const live = {
  gateways: s.gateways,
  aircraft: s.aircraft,
  tracks: s.active_flights.map((f) => f.segments),
  serverNow: s.server_time_ms,
  thresholds: s.freshness,
};
// Beide mit `theme`, sonst baut antd den Baum um und die Karte würde neu eingehängt statt umgefärbt.
const light = (ui: ReactElement) => (
  <ConfigProvider theme={{ algorithm: theme.defaultAlgorithm }}>{ui}</ConfigProvider>
);
const dark = (ui: ReactElement) => (
  <ConfigProvider theme={{ algorithm: theme.darkAlgorithm }}>{ui}</ConfigProvider>
);
const layout = (map: InstanceType<typeof FakeMap>, id: string) =>
  map.addLayer.mock.calls.map(([l]) => l).find((l) => l.id === id).layout;
const loaded = () => {
  const map = maps.at(-1)!;
  act(() => map.handlers.get('load')!({}));
  return map;
};

beforeEach(() => {
  maps.length = 0;
  gpu.available = true;
  gpu.fault = null;
});
afterEach(() => {
  vi.restoreAllMocks();
});

test('zeigt Fluggerät, Trupp, Spur und Startpunkt der Lage', () => {
  render(light(<SituationMap {...live} />));
  const map = loaded();
  expect(map.features('aircraft')).toHaveLength(1);
  expect(map.features('gateways')[0].properties.label).toBe('RC Plus 1');
  expect(map.features('track')).toHaveLength(2);
  expect(map.features('home')).toHaveLength(1);
});

test('holt die Lage einmal ins Bild, danach bestimmt der Nutzer den Ausschnitt', () => {
  const { rerender } = render(light(<SituationMap {...live} />));
  const map = loaded();
  // Der Trupp steht südwestlich der Spur und gehört mit ins Bild.
  expect(map.fitBounds).toHaveBeenCalledTimes(1);
  expect(map.fitBounds.mock.calls[0]![0]).toEqual([
    [10.0097, 52.4296],
    [10.0134, 52.4321],
  ]);
  rerender(light(<SituationMap {...live} serverNow={live.serverNow + 1_000} />));
  expect(map.fitBounds).toHaveBeenCalledTimes(1);
});

test('Folgen holt die Lage zuerst ins Bild und führt die Karte dann mit dem Fluggerät', () => {
  const { rerender } = render(light(<SituationMap {...live} follow />));
  const map = loaded();
  // Ohne erstes Einpassen bliebe die Karte auf der Deutschland-Übersicht stehen.
  expect(map.fitBounds).toHaveBeenCalledTimes(1);
  expect(map.easeTo).not.toHaveBeenCalled();
  const a = s.aircraft[0]!;
  const moved = { ...a, telemetry: { ...a.telemetry, position: { ...a.telemetry.position!, lon: 10.015 } } };
  rerender(light(<SituationMap {...live} aircraft={[moved]} follow />));
  expect(map.easeTo.mock.calls.at(-1)![0].center).toEqual([10.015, 52.4321]);
  expect(map.fitBounds).toHaveBeenCalledTimes(1);
});

test('Folgen ist ein Schalter', async () => {
  const onFollowChange = vi.fn();
  render(light(<SituationMap {...live} onFollowChange={onFollowChange} />));
  const button = screen.getByRole('button', { name: 'Folgen' });
  expect(button).toHaveAttribute('aria-pressed', 'false');
  await userEvent.click(button);
  expect(onFollowChange).toHaveBeenCalledWith(true);
});

test('wer die Karte selbst verschiebt, beendet das Folgen', () => {
  const onFollowChange = vi.fn();
  render(light(<SituationMap {...live} follow onFollowChange={onFollowChange} />));
  const map = loaded();
  act(() => map.handlers.get('dragstart')!({}));
  expect(onFollowChange).not.toHaveBeenCalled();
  act(() => map.handlers.get('dragstart')!({ originalEvent: new MouseEvent('mousedown') }));
  expect(onFollowChange).toHaveBeenCalledWith(false);
});

test('Großbild: Live-Zeichen ohne Kartenknöpfe', () => {
  render(light(<SituationMap {...live} follow showTools={false} />));
  const map = loaded();
  expect(map.features('aircraft')).toHaveLength(1);
  expect(screen.queryAllByRole('button')).toHaveLength(0);
});

test('der Wechsel Hell/Dunkel behält den Ausschnitt', () => {
  const { rerender } = render(light(<SituationMap {...live} />));
  const first = loaded();
  first.center = { lng: 10.02, lat: 52.44 };
  first.zoom = 15.5;
  rerender(dark(<SituationMap {...live} />));
  expect(first.remove).toHaveBeenCalled();
  expect(maps).toHaveLength(2);
  expect(maps[1]!.options).toMatchObject({ center: { lng: 10.02, lat: 52.44 }, zoom: 15.5 });
  loaded();
  expect(maps[1]!.fitBounds).not.toHaveBeenCalled();
});

test('Flugdetail zeigt nur die Spur, ohne Live-Zeichen und ohne Bedienung', () => {
  render(light(<SituationMap {...live} staticFit />));
  const map = loaded();
  expect(map.features('aircraft')).toHaveLength(0);
  expect(map.features('gateways')).toHaveLength(0);
  expect(map.fitBounds.mock.calls[0]![0]).toEqual([
    [10.01, 52.43],
    [10.0134, 52.4321],
  ]);
  expect(screen.queryByRole('button', { name: 'Folgen' })).not.toBeInTheDocument();
});

test('jede Frische hat ihre Kursglyphe: frisch und verzögert gefüllt, verloren nur als Kontur', () => {
  render(light(<SituationMap {...live} />));
  const map = loaded();
  // Record statt Liste: Kommt eine Frische hinzu, fehlt sie hier schon beim Übersetzen.
  const glyph: Record<Freshness, { color: string; outlineOnly: boolean }> = {
    fresh: { color: farbenHell.bedien, outlineOnly: false },
    delayed: { color: farbenHell.gedaempft, outlineOnly: false },
    lost: { color: farbenHell.schwach, outlineOnly: true },
  };
  for (const [freshness, image] of Object.entries(glyph)) {
    expect(map.addImage).toHaveBeenCalledWith(`aircraft-${freshness}`, image, expect.anything());
  }
  expect(layout(map, 'aircraft')['icon-image']).toEqual(['concat', 'aircraft-', ['get', 'freshness']]);
});

test('Zeichen bleiben stehen, auch wenn ihre Beschriftungen einander überdecken', () => {
  render(light(<SituationMap {...live} />));
  const map = loaded();
  // Drohne und Fernsteuerung stehen am Boden wenige Meter auseinander.
  for (const id of ['gateways', 'aircraft']) {
    expect(layout(map, id)).toMatchObject({ 'icon-allow-overlap': true, 'text-allow-overlap': true });
  }
});

test('ohne WebGL 2 steht ein Hinweis statt der Karte, und die Seite bleibt stehen', () => {
  vi.spyOn(console, 'error').mockImplementation(() => {});
  gpu.available = false;
  const { rerender } = render(light(<SituationMap {...live} />));
  const alert = screen.getByRole('alert');
  expect(alert).toHaveTextContent(MAP_UNAVAILABLE_TITLE);
  expect(alert).toHaveTextContent(MAP_UNAVAILABLE_TEXT);
  expect(screen.getByRole('region', { name: 'Lagekarte' })).toContainElement(alert);
  expect(screen.queryByRole('button', { name: 'Folgen' })).not.toBeInTheDocument();
  // Auch der Wechsel Hell/Dunkel versucht es nicht erneut in einem Behälter, den es nicht mehr gibt.
  rerender(dark(<SituationMap {...live} />));
  expect(screen.getByRole('alert')).toHaveTextContent(MAP_UNAVAILABLE_TITLE);
});

test('ein Fehler im Code bleibt ein Fehler und geht an die Fehlergrenze der Seite', () => {
  vi.spyOn(console, 'error').mockImplementation(() => {});
  gpu.fault = new TypeError('kaputt');
  expect(() => render(light(<SituationMap {...live} />))).toThrow('kaputt');
});
