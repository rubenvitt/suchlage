import { drawSymbol } from '@einsatzzeichen/core';
import { DRONE_TEAM_SPEC } from './unitSign';

test('„Trupp Drohne“ (I.1.20) ist mit einsatzzeichen 6.4 zeichenbar', () => {
  const drawing = drawSymbol(DRONE_TEAM_SPEC);
  expect(drawing.viewBox.width).toBeGreaterThan(0);
});
