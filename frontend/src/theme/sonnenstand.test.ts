import { sonneUeberHorizont } from './sonnenstand';

const t = (iso: string) => Date.parse(iso);

test('Sonnenstand in Bremen über die Jahreszeiten', () => {
  // Bremen, 01.10.2026: Aufgang etwa 05:24Z, Untergang etwa 17:05Z.
  const bremen = { lat: 53.08, lon: 8.8 };
  expect(sonneUeberHorizont(bremen, t('2026-10-01T05:00:00Z'))).toBe(false);
  expect(sonneUeberHorizont(bremen, t('2026-10-01T06:00:00Z'))).toBe(true);
  expect(sonneUeberHorizont(bremen, t('2026-10-01T16:30:00Z'))).toBe(true);
  expect(sonneUeberHorizont(bremen, t('2026-10-01T17:30:00Z'))).toBe(false);
  // Mittag und Mitternacht im Sommer und im Winter.
  expect(sonneUeberHorizont(bremen, t('2026-06-21T11:30:00Z'))).toBe(true);
  expect(sonneUeberHorizont(bremen, t('2026-06-21T23:30:00Z'))).toBe(false);
  expect(sonneUeberHorizont(bremen, t('2026-12-21T11:30:00Z'))).toBe(true);
  expect(sonneUeberHorizont(bremen, t('2026-12-21T23:30:00Z'))).toBe(false);
  // 21.12.: Aufgang etwa 07:37Z. Um 07:40Z steht die Sonne bei etwa −0,4°, also schon Tag.
  expect(sonneUeberHorizont(bremen, t('2026-12-21T07:40:00Z'))).toBe(true);
  expect(sonneUeberHorizont(bremen, t('2026-12-21T07:15:00Z'))).toBe(false);
});

test('der Ort zählt: im Osten geht die Sonne früher auf', () => {
  // 27.10.2026 kurz nach 06:00Z: in Görlitz schon hell, in Aachen noch dunkel.
  const zeit = t('2026-10-27T06:05:00Z');
  expect(sonneUeberHorizont({ lat: 51.15, lon: 14.99 }, zeit)).toBe(true);
  expect(sonneUeberHorizont({ lat: 50.78, lon: 6.08 }, zeit)).toBe(false);
});
