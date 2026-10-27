import { render, renderHook } from '@testing-library/react';
import type { ReactNode } from 'react';
import type { AircraftState } from '../contract/generated/AircraftState';
import type { GatewayState } from '../contract/generated/GatewayState';
import type { MapElement } from '../contract/generated/MapElement';
import { ThemeModeProvider } from '../theme/ThemeModeProvider';
import { ortDerLage, useSonnenTheme } from './useSonnenTheme';

const MITTAG = Date.parse('2026-10-27T12:00:00Z');
const ABEND = Date.parse('2026-10-27T20:00:00Z');

const punkt = (name: string, lat: number, lon: number) =>
  ({ name, kind: 'point', points: [{ lat, lon, alt_m: null }] }) as MapElement;
const fernsteuerung = (lat: number, lon: number) => ({ position: { lat, lon } }) as GatewayState;
const fluggeraet = (home: { lat: number; lon: number } | null, position: { lat: number; lon: number } | null) =>
  ({ telemetry: { home, position } }) as AircraftState;

const leer = { elements: [], gateways: [], aircraft: [] };

beforeEach(() => localStorage.clear());

test('der Standort des Einsatzes geht vor Fernsteuerung und Fluggerät', () => {
  expect(
    ortDerLage({
      elements: [punkt('Suchgebiet Nord', 1, 1), punkt('Standort', 52.97, 10.56)],
      gateways: [fernsteuerung(53.5, 10)],
      aircraft: [fluggeraet({ lat: 54, lon: 9 }, null)],
    }),
  ).toEqual({ lat: 52.97, lon: 10.56, alt_m: null });
  expect(
    ortDerLage({ elements: [], gateways: [fernsteuerung(53.5, 10)], aircraft: [fluggeraet({ lat: 54, lon: 9 }, null)] }),
  ).toEqual({ lat: 53.5, lon: 10 });
  expect(ortDerLage({ elements: [], gateways: [], aircraft: [fluggeraet(null, { lat: 54, lon: 9 })] })).toEqual({
    lat: 54,
    lon: 9,
  });
  expect(ortDerLage(leer)).toBeNull();
});

function wrapper({ children }: { children: ReactNode }) {
  return <ThemeModeProvider>{children}</ThemeModeProvider>;
}

test('Großbild ist bei Tag hell und wechselt abends ohne Neuladen auf dunkel', () => {
  const quellen = { ...leer, gateways: [fernsteuerung(52.97, 10.56)] };
  const { rerender, unmount } = renderHook(({ jetzt }) => useSonnenTheme(quellen, jetzt), {
    wrapper,
    initialProps: { jetzt: MITTAG },
  });
  expect(document.documentElement.dataset.theme).toBe('light');
  rerender({ jetzt: ABEND });
  expect(document.documentElement.dataset.theme).toBe('dark');
  rerender({ jetzt: MITTAG });
  expect(document.documentElement.dataset.theme).toBe('light');
  // Die Wahl des Geräts bleibt stehen.
  unmount();
  expect(localStorage.getItem('suchlage.theme')).toBeNull();
});

test('ohne Großbild gilt wieder die Wahl des Geräts', () => {
  localStorage.setItem('suchlage.theme', 'dark');
  function Grossbild() {
    useSonnenTheme({ ...leer, gateways: [fernsteuerung(52.97, 10.56)] }, MITTAG);
    return null;
  }
  const { rerender } = render(<Grossbild />, { wrapper });
  expect(document.documentElement.dataset.theme).toBe('light');
  rerender(<p>Arbeitsplatz</p>);
  expect(document.documentElement.dataset.theme).toBe('dark');
});

test('ohne Ort in der Lage rechnet das Großbild mit dem zuletzt bekannten Ort', () => {
  // 27.10., 06:05Z: in Görlitz schon hell, in Aachen noch dunkel.
  const frueh = Date.parse('2026-10-27T06:05:00Z');
  const erst = renderHook(() => useSonnenTheme({ ...leer, gateways: [fernsteuerung(51.15, 14.99)] }, frueh), {
    wrapper,
  });
  expect(document.documentElement.dataset.theme).toBe('light');
  erst.unmount();
  const zweit = renderHook(() => useSonnenTheme(leer, frueh), { wrapper });
  expect(document.documentElement.dataset.theme).toBe('light');
  zweit.unmount();
  localStorage.setItem('suchlage.sonnenort', JSON.stringify({ lat: 50.78, lon: 6.08 }));
  renderHook(() => useSonnenTheme(leer, frueh), { wrapper });
  expect(document.documentElement.dataset.theme).toBe('dark');
});
