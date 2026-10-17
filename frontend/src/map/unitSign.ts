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
/** Kantenlänge des Zeichens in CSS-Pixeln. */
export const UNIT_SIZE = 36;

/**
 * Gerätedichte auf Viertel gerundet: einsatzzeichen verlangt eine ganzzahlige Rasterbreite, und 36 × ¼
 * ist exakt ganzzahlig. Mit 2,625 (Pixel 7) oder 1,1 (Browser-Zoom 110 %) wäre sie es nicht.
 */
export const unitPixelRatio = (pixelRatio: number) => Math.max(1, Math.round(pixelRatio * 4)) / 4;

export function registerUnitSign(map: MapLike, pixelRatio: number) {
  if (map.hasImage(UNIT_IMAGE)) return;
  addSymbolImage(map, UNIT_IMAGE, drawSymbol(DRONE_TEAM_SPEC, { title: 'Trupp Drohne' }), {
    size: UNIT_SIZE,
    pixelRatio: unitPixelRatio(pixelRatio),
  });
}
