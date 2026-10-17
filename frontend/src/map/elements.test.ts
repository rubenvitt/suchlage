import { mapElements } from '../contract/fixtures.generated';
import type { MapElement } from '../contract/generated/MapElement';
import {
  addVertex,
  canFinish,
  defaultName,
  draftCollection,
  draftHint,
  elementCollection,
  elementLabelCollection,
  removeLastVertex,
  startDraft,
} from './elements';

const p = (lat: number, lon: number) => ({ lat, lon });

test('eine Fläche braucht drei Eckpunkte, eine Linie zwei, ein Punkt einen', () => {
  let area = startDraft('polygon');
  area = addVertex(addVertex(area, p(52, 10)), p(52.1, 10));
  expect(canFinish(area)).toBe(false);
  area = addVertex(area, p(52.1, 10.1));
  expect(canFinish(area)).toBe(true);
  expect(canFinish(removeLastVertex(area))).toBe(false);
  expect(canFinish(addVertex(startDraft('line'), p(52, 10)))).toBe(false);
  expect(canFinish(addVertex(startDraft('point'), p(52, 10)))).toBe(true);
});

test('beim Punkt ersetzt ein weiterer Klick die Koordinate', () => {
  const pin = addVertex(addVertex(startDraft('point'), p(52, 10)), p(53, 11));
  expect(pin.points).toEqual([p(53, 11)]);
});

test('die Vorschau zeigt Eckpunkte und ab zwei Punkten die Form', () => {
  expect(draftCollection(null).features).toEqual([]);
  const one = addVertex(startDraft('polygon'), p(52, 10));
  expect(draftCollection(one).features.map((f) => f.geometry.type)).toEqual(['Point']);
  const two = addVertex(one, p(52.1, 10));
  // Mit zwei Ecken ist die Fläche noch eine Linie.
  expect(draftCollection(two).features.map((f) => f.geometry.type)).toEqual(['LineString', 'Point', 'Point']);
  const three = addVertex(two, p(52.1, 10.1));
  expect(draftCollection(three).features[0]!.geometry.type).toBe('Polygon');
});

test('leere Formen fallen aus der Karte, statt sie zu zerbrechen', () => {
  const broken: MapElement = { ...mapElements[0]!, points: [] };
  expect(elementCollection([broken, mapElements[1]!]).features).toHaveLength(1);
});

test('der Namensvorschlag zählt je Form weiter', () => {
  expect(defaultName('polygon', mapElements)).toBe('Fläche 1');
  const two = [
    { ...mapElements[0]!, name: 'Fläche 1' },
    { ...mapElements[0]!, name: 'Fläche 4' },
  ];
  expect(defaultName('polygon', two)).toBe('Fläche 5');
  expect(defaultName('point', two)).toBe('Punkt 1');
});

test('der Hinweis sagt, was noch fehlt', () => {
  const area = addVertex(startDraft('polygon'), p(52, 10));
  expect(draftHint(area)).toBe('1 Eckpunkt · noch mindestens 2 setzen');
  const line = addVertex(addVertex(startDraft('line'), p(52, 10)), p(52, 11));
  expect(draftHint(line)).toBe('2 Punkte · „Fertig“ oder Eingabetaste schließt ab');
});

test('je Fläche und Punkt genau eine Beschriftung, Linien beschriftet die Linie selbst', () => {
  const line: MapElement = {
    ...mapElements[0]!,
    id: 'l',
    kind: 'line',
    points: mapElements[0]!.points.slice(0, 2),
  };
  const labels = elementLabelCollection([...mapElements, line]).features;
  expect(labels.map((f) => [f.properties.name, f.properties.kind])).toEqual([
    ['Suchgebiet Nord', 'polygon'],
    ['Pilot 1', 'point'],
  ]);
  // Die Mitte der vier Ecken des Beispielrechtecks.
  expect(labels[0]!.geometry.coordinates[0]).toBeCloseTo(10.012, 6);
  expect(labels[0]!.geometry.coordinates[1]).toBeCloseTo(52.43175, 6);
});
