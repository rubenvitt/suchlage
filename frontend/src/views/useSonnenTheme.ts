import { useEffect, useMemo } from 'react';
import type { AircraftState } from '../contract/generated/AircraftState';
import type { GatewayState } from '../contract/generated/GatewayState';
import type { MapElement } from '../contract/generated/MapElement';
import { sicherLesen, sicherSchreiben } from '../lib/sichererSpeicher';
import { sonneUeberHorizont, type Ort } from '../theme/sonnenstand';
import { useThemeVorgabe } from '../theme/ThemeModeProvider';

/** Name des Punkts, den die Flugmappe als Standort des Einsatzes anlegt (`missions::rules::STANDORT_NAME`). */
const STANDORT_NAME = 'Standort';
/** Der zuletzt bekannte Ort: Nach einem Neuladen ohne Lage rechnet das Großbild mit ihm weiter. */
const ORT_SCHLUESSEL = 'suchlage.sonnenort';
/** Ohne jeden Ort: Mitte Deutschlands. Auf- und Untergang liegen dann um höchstens etwa eine halbe Stunde daneben. */
const ORT_VORGABE: Ort = { lat: 51.16, lon: 10.45 };

interface Quellen {
  elements: readonly MapElement[];
  gateways: readonly GatewayState[];
  aircraft: readonly AircraftState[];
}

const gueltig = (p: Ort | null | undefined): p is Ort =>
  !!p && Number.isFinite(p.lat) && Number.isFinite(p.lon) && Math.abs(p.lat) <= 90 && Math.abs(p.lon) <= 180;

/**
 * Wo das Großbild steht, so gut die Lage es weiß: der Standort des Einsatzes aus der Flugmappe,
 * sonst die Fernsteuerung (beim Piloten, nah am FüKw), sonst Startpunkt oder Position des
 * Fluggeräts. `null`, wenn die Lage keinen Ort kennt. Rein.
 */
export function ortDerLage({ elements, gateways, aircraft }: Quellen): Ort | null {
  const standort = elements.find((e) => e.kind === 'point' && e.name === STANDORT_NAME)?.points[0];
  if (gueltig(standort)) return standort;
  const fernsteuerung = gateways.map((g) => g.position).find(gueltig);
  if (fernsteuerung) return fernsteuerung;
  return aircraft.flatMap((a) => [a.telemetry.home, a.telemetry.position]).find(gueltig) ?? null;
}

function gespeicherterOrt(): Ort | null {
  try {
    const ort = JSON.parse(sicherLesen(ORT_SCHLUESSEL) ?? 'null') as Ort | null;
    return gueltig(ort) ? ort : null;
  } catch {
    return null;
  }
}

/**
 * Großbild: hell zwischen Sonnenauf- und -untergang am Ort der Lage, sonst dunkel. Überstimmt die
 * gespeicherte Wahl des Geräts nur, solange die Ansicht steht. `jetztMs` ist die Serveruhr; das
 * Theme wechselt, sobald sie die Grenze überschreitet, ohne Neuladen.
 */
export function useSonnenTheme(quellen: Quellen, jetztMs: number) {
  const lage = ortDerLage(quellen);
  // Auf zwei Stellen gerundet (etwa 1 km): Der Speicher ändert sich nicht mit jeder Telemetrie.
  const lat = lage ? Math.round(lage.lat * 100) / 100 : null;
  const lon = lage ? Math.round(lage.lon * 100) / 100 : null;
  useEffect(() => {
    if (lat !== null && lon !== null) sicherSchreiben(ORT_SCHLUESSEL, JSON.stringify({ lat, lon }));
  }, [lat, lon]);
  const ort = useMemo(
    () => (lat !== null && lon !== null ? { lat, lon } : (gespeicherterOrt() ?? ORT_VORGABE)),
    [lat, lon],
  );
  useThemeVorgabe(sonneUeberHorizont(ort, jetztMs) ? 'light' : 'dark');
}
