import { ageLabel, freshnessOf } from './freshness';

const t = { delayed_after_ms: 6_000, lost_after_ms: 30_000 };

test('Frische nach den Schwellen des Servers', () => {
  expect(freshnessOf(0, 6_000, t)).toBe('fresh');
  expect(freshnessOf(0, 6_001, t)).toBe('delayed');
  expect(freshnessOf(0, 30_000, t)).toBe('delayed');
  expect(freshnessOf(0, 30_001, t)).toBe('lost');
});

test('Alter in lesbarer Form', () => {
  expect(ageLabel(900)).toBe('vor 1 s');
  expect(ageLabel(9_400)).toBe('vor 9 s');
  expect(ageLabel(125_000)).toBe('vor 2 min');
  expect(ageLabel(7_500_000)).toBe('vor 2 h');
});
