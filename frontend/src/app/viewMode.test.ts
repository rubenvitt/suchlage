import { viewModeFor } from './viewMode';

test('Großbild-Rolle ist immer Kiosk, schmale Bildschirme sind mobil', () => {
  expect(viewModeFor('kiosk', 1920)).toBe('kiosk');
  expect(viewModeFor('kiosk', 400)).toBe('kiosk');
  expect(viewModeFor('operator', 767)).toBe('mobile');
  expect(viewModeFor('admin', 768)).toBe('workstation');
});
