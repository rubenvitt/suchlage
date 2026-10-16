import { drawSymbol } from '@einsatzzeichen/core';
import { addSymbolImage, type MapLike } from '@einsatzzeichen/maplibre';
import type { SymbolSpec } from '@einsatzzeichen/schema';

/** I.1.20 „Trupp Drohne“ einer Hilfsorganisation (geprüft mit 6.4.0; `drk` ist keine Organisation der Bibliothek). */
export const DRONE_TEAM_SPEC: SymbolSpec = {
  kind: 'formation',
  organization: 'hilfsorganisation',
  strength: 'trupp',
  bodyMarks: ['formation-chevron-top'],
};

export const UNIT_IMAGE = 'unit-drone-team';

export function registerUnitSign(map: MapLike, pixelRatio: number) {
  if (map.hasImage(UNIT_IMAGE)) return;
  addSymbolImage(map, UNIT_IMAGE, drawSymbol(DRONE_TEAM_SPEC, { title: 'Trupp Drohne' }), {
    size: 36,
    pixelRatio,
  });
}
