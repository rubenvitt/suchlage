import { flightList, mapElements, photoShots } from '../contract/fixtures.generated';

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
