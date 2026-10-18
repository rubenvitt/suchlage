import { flightList, mapElements, missionList, photoShots } from '../contract/fixtures.generated';
import type { Mission } from '../contract/generated/Mission';

/** Vorschau: Der laufende Beispiel-Einsatz hat die Nummer 1; je ein Element, Foto und Flug bleibt ohne Einsatz. */
export const demoElements = mapElements.map((e, i) => ({
  ...e,
  mission_id: i === mapElements.length - 1 ? null : 1,
}));
export const demoPhotos = photoShots.map((s, i) => ({
  ...s,
  mission_id: i === photoShots.length - 1 ? null : 1,
}));
export const demoFlightMissions = (): Map<number, number | null> =>
  new Map(flightList.map((f, i) => [f.id, i === flightList.length - 1 ? null : 1]));

/** `?mission=<id>` oder `none` wie bei den Server-Routen. */
export const matchesMission = (url: string, missionId: number | null): boolean => {
  const q = new URL(url).searchParams.get('mission');
  if (q === null) return true;
  return q === 'none' ? missionId === null : missionId === Number(q);
};

/**
 * Vorschau: ein abgeschlossener Einsatz aus der Flugmappe mit Kartenfoto am zweiten Abschnitt. Er
 * steht im Archiv, weil der laufende Beispiel-Einsatz (Nummer 1) in Suchlage angelegt ist.
 */
export const demoFlugmappeMission = (): Mission => {
  const fm = missionList.find((m) => m.origin === 'flugmappe')!;
  return {
    ...fm,
    id: 3,
    state: 'closed',
    closed_at_ms: 1792149900000,
    sections: fm.sections.map((s) => ({ ...s })),
  };
};
