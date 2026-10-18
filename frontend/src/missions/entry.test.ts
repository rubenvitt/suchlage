import { act, renderHook } from '@testing-library/react';
import { missionFixtures } from '../test/handlers';
import { entryFor, SKIP_KEY, useSkip } from './entry';

const [running, prepared] = missionFixtures();
const base = { role: 'operator' as const, missions: [prepared!], param: null, skipped: false };

afterEach(() => sessionStorage.clear());

test('Einstieg: Großbild, laufender Einsatz, Adresse und Überspringen führen in die Lage', () => {
  expect(entryFor({ ...base, role: 'kiosk' })).toBe('lage');
  expect(entryFor({ ...base, missions: [running!, prepared!] })).toBe('lage');
  expect(entryFor({ ...base, param: '2' })).toBe('lage');
  expect(entryFor({ ...base, param: 'ohne' })).toBe('lage');
  expect(entryFor({ ...base, skipped: true })).toBe('lage');
});

test('Einstieg: sonst die Startseite', () => {
  expect(entryFor(base)).toBe('welcome');
  expect(entryFor({ ...base, role: 'admin', missions: [] })).toBe('welcome');
});

test('Überspringen gilt für die Browsersitzung und für alle Leser', () => {
  const a = renderHook(() => useSkip());
  const b = renderHook(() => useSkip());
  expect(a.result.current.skipped).toBe(false);
  act(() => a.result.current.skip());
  expect(b.result.current.skipped).toBe(true);
  expect(sessionStorage.getItem(SKIP_KEY)).toBe('1');
  act(() => b.result.current.reset());
  expect(a.result.current.skipped).toBe(false);
  expect(sessionStorage.getItem(SKIP_KEY)).toBeNull();
});

test('ohne Sitzungsspeicher wirkt Überspringen trotzdem', () => {
  const blocked = vi.spyOn(Storage.prototype, 'setItem').mockImplementation(() => {
    throw new Error('gesperrt');
  });
  const read = vi.spyOn(Storage.prototype, 'getItem').mockImplementation(() => {
    throw new Error('gesperrt');
  });
  try {
    const { result } = renderHook(() => useSkip());
    act(() => result.current.skip());
    expect(result.current.skipped).toBe(true);
    act(() => result.current.reset());
    expect(result.current.skipped).toBe(false);
  } finally {
    blocked.mockRestore();
    read.mockRestore();
  }
});
