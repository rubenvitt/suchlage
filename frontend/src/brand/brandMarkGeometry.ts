/**
 * Bildmarke „Suchspirale“: das Zeichen von Suchlage, Geschwister der Lifeline-Marke „Lebenslinie“.
 *
 * Ein Quadrat in `marke` steht für den letzten bekannten Ort der vermissten Person. Von ihm geht die
 * Linie als Suchmuster „Expanding Square“ nach außen, so wie eine Drohne den Ort absucht. Lifeline
 * lässt die Linie im Quadrat enden, hier beginnt sie dort.
 *
 * EINE Geometrie für die ganze App. `BrandMark.tsx` zeichnet daraus die Marke der Kopfleiste,
 * `public/favicon.svg` bindet denselben Pfad über `<g transform>` ein, und
 * `scripts/render-icons.ts` rendert daraus `apple-touch-icon.png`. `brandMark.guard.test.tsx` hält
 * das Favicon wörtlich auf diesen Werten.
 *
 * Lokales Raster: das Quadrat liegt mittig auf (0, 0), die Schritte der Spirale sind 110 lang
 * (1, 1, 2, 2, 3, 3, 3). Die Ecken sind rechtwinklige Gehrungen (Radius 0 der Gestaltungssprache).
 */

/** Suchspirale: vom Quadrat nach rechts, dann gegen den Uhrzeigersinn nach außen. */
export const LINE_PATH = 'M36 0H110V-110H-110V110H220V-220H-110';
export const LINE_WIDTH = 54;
export const LINE_JOIN = 'miter';
export const LINE_MITER_LIMIT = 10;

/** Das Quadrat in `marke`; die Linie setzt stumpf an seiner rechten Kante an. */
export const SQUARE = { x: -36, y: -36, size: 72 } as const;

/** Enges Rechteck um die gezeichnete Marke: äußere Schenkel ± halbe Strichstärke. */
export const MARK_BOX = { x: -137, y: -247, width: 384, height: 384 } as const;
export const MARK_VIEWBOX = `${MARK_BOX.x} ${MARK_BOX.y} ${MARK_BOX.width} ${MARK_BOX.height}`;
