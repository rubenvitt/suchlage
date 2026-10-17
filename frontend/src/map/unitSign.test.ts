import { drawSymbol } from '@einsatzzeichen/core';
import { DRONE_TEAM_SPEC, UNIT_SIZE, unitPixelRatio } from './unitSign';

test('„Trupp Drohne“ (I.1.20) ist mit einsatzzeichen 6.4 zeichenbar', () => {
  const drawing = drawSymbol(DRONE_TEAM_SPEC);
  expect(drawing.viewBox.width).toBeGreaterThan(0);
});

test.each([1, 1.25, 1.5, 1.75, 2, 2.625, 2.75, 3, 3.5, 0.9, 1.1, 1.333])(
  'Pixelverhältnis %s ergibt ein ganzzahliges Raster nahe der Gerätedichte',
  (devicePixelRatio) => {
    const ratio = unitPixelRatio(devicePixelRatio);
    // einsatzzeichen lehnt Rasterbreiten mit Nachkommastellen ab; dann fehlte die ganze Lage auf der Karte.
    expect(Number.isInteger(UNIT_SIZE * ratio)).toBe(true);
    expect(Math.abs(ratio - devicePixelRatio)).toBeLessThanOrEqual(0.125);
  },
);
