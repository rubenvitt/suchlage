import type { Mission } from '../contract/generated/Mission';
import { inScope, resolveScope, scopeQuery } from './scope';

const mission = (id: number, state: Mission['state']): Mission => ({
  id,
  name: `Einsatz ${id}`,
  location: null,
  state,
  created_by: 'Test',
  created_at_ms: 0,
  started_at_ms: null,
  closed_at_ms: null,
  sections: [],
  origin: 'web',
  flugmappe_links: [],
});
const m7 = mission(7, 'prepared');
const running = mission(3, 'running');

test('Auswahl: bekannte Zahl, „ohne“, sonst der laufende Einsatz', () => {
  expect(resolveScope('7', [m7])).toEqual({ kind: 'mission', mission: m7 });
  expect(resolveScope('999', [running])).toEqual({ kind: 'mission', mission: running });
  expect(resolveScope('abc', [running])).toEqual({ kind: 'mission', mission: running });
  expect(resolveScope(null, [running, m7])).toEqual({ kind: 'mission', mission: running });
  expect(resolveScope('ohne', [running])).toEqual({ kind: 'none' });
  expect(resolveScope(null, [])).toEqual({ kind: 'none' });
});

test('Abfragewert und Zugehörigkeit', () => {
  expect(scopeQuery({ kind: 'mission', mission: m7 })).toBe('7');
  expect(scopeQuery({ kind: 'none' })).toBe('none');
  expect(inScope(7, { kind: 'mission', mission: m7 })).toBe(true);
  expect(inScope(null, { kind: 'mission', mission: m7 })).toBe(false);
  expect(inScope(null, { kind: 'none' })).toBe(true);
  expect(inScope(7, { kind: 'none' })).toBe(false);
});
