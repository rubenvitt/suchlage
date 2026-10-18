import { photoShots } from '../contract/fixtures.generated';
import { mergeShots, photoCollection, shotTitle } from './photos';

const [withPlace, withoutPlace] = photoShots as [(typeof photoShots)[0], (typeof photoShots)[0]];

test('Liste und Strom zusammen: der Strom gewinnt, sortiert nach Aufnahmezeit', () => {
  const newer = { ...withPlace, photos: [] };
  const merged = mergeShots([withoutPlace, withPlace], { [withPlace.id]: newer });
  expect(merged.map((s) => s.id)).toEqual([withPlace.id, withoutPlace.id]);
  expect(merged[0]).toBe(newer);
  expect(mergeShots(undefined, {})).toEqual([]);
});

test('auf die Karte nur mit Aufnahmeort, die gewählte markiert', () => {
  const c = photoCollection(photoShots, withPlace.id);
  expect(c.features).toHaveLength(1);
  expect(c.features[0]!.geometry.coordinates).toEqual([withPlace.position!.lon, withPlace.position!.lat]);
  expect(c.features[0]!.properties).toMatchObject({ id: withPlace.id, selected: true });
  expect(c.features[0]!.properties.time).toMatch(/^\d\d:\d\d$/);
});

test('Titel nennt Uhrzeit und Höhe über Start, wenn bekannt', () => {
  expect(shotTitle(withPlace)).toMatch(/^\d\d:\d\d · 62 m über Start$/);
  expect(shotTitle(withoutPlace)).toMatch(/^\d\d:\d\d$/);
});
