// Übernommen aus Lifeline Hub 3eb9115f2 (16.10.2026): frontend/src/test/farbmass.ts
/**
 * Farbmaße für Tests, gerechnet statt behauptet: relative Leuchtdichte und Kontrast nach WCAG 2.x,
 * CIELAB (D65) für Helligkeit L* und Farbabstand ΔE (CIE76). Eine Quelle für die Farbtests
 * (`theme/textstufen.test.ts`, `theme/rahmenKontrast.test.ts`, `theme/statusFarben.test.ts`,
 * `components/instrument/statusFlaeche.test.ts`).
 *
 * Nimmt nur deckende Farben als `#rrggbb`; Durchscheinendes ist kein Textgrund und wird im
 * Browser gegen den tatsächlich gerenderten Grund gemessen (`e2e/kontrast-kern.ts`).
 */

function kanaele(hex: string): [number, number, number] {
  if (!/^#[0-9a-f]{6}$/i.test(hex)) throw new Error(`Keine deckende Farbe #rrggbb: ${hex}`);
  return [1, 3, 5].map((i) => {
    const s = Number.parseInt(hex.slice(i, i + 2), 16) / 255;
    return s <= 0.04045 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
  }) as [number, number, number];
}

/** Relative Leuchtdichte nach WCAG 2.x. */
export function luminanz(hex: string): number {
  const [r, g, b] = kanaele(hex);
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

/** Kontrastverhältnis nach WCAG 2.x, unabhängig von der Reihenfolge. */
export function kontrast(a: string, b: string): number {
  const [x, y] = [luminanz(a), luminanz(b)];
  return (Math.max(x, y) + 0.05) / (Math.min(x, y) + 0.05);
}

/** CIELAB (D65). */
export function lab(hex: string): [number, number, number] {
  const [r, g, b] = kanaele(hex);
  const f = (t: number) => (t > 0.008856 ? Math.cbrt(t) : 7.787 * t + 16 / 116);
  const x = f((r * 0.4124 + g * 0.3576 + b * 0.1805) / 0.95047);
  const y = f(r * 0.2126 + g * 0.7152 + b * 0.0722);
  const z = f((r * 0.0193 + g * 0.1192 + b * 0.9505) / 1.08883);
  return [116 * y - 16, 500 * (x - y), 200 * (y - z)];
}

/** Farbabstand ΔE (CIE76) in CIELAB. */
export function abstand(a: string, b: string): number {
  const [p, q] = [lab(a), lab(b)];
  return Math.hypot(p[0] - q[0], p[1] - q[1], p[2] - q[2]);
}
