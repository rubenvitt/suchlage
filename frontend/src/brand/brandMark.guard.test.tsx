import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { render } from '@testing-library/react';
import { farbenDunkel } from '../theme/tokens';
import { BrandMark } from './BrandMark';
import {
  LINE_JOIN,
  LINE_MITER_LIMIT,
  LINE_PATH,
  LINE_WIDTH,
  MARK_BOX,
  SQUARE,
} from './brandMarkGeometry';

/**
 * Guard der Bildmarke „Suchspirale“: Das Favicon zeichnet dieselbe Geometrie wie die Kopfleiste,
 * das Symbol für iPad/iPhone hat seine Nenngröße, und `MARK_BOX` umschließt die Marke. Schlägt ein
 * Fall fehl: Geometrie in `brandMarkGeometry.ts` ändern, `public/favicon.svg` nachziehen,
 * `node scripts/render-icons.ts` laufen lassen.
 */

const frontend = join(dirname(fileURLToPath(import.meta.url)), '..', '..');
const publicDir = join(frontend, 'public');

/** Breite und Höhe aus dem IHDR-Block einer PNG-Datei. */
function pngSize(path: string) {
  const data = readFileSync(path);
  expect(data.subarray(12, 16).toString('ascii'), `${path} ist keine PNG`).toBe('IHDR');
  return { width: data.readUInt32BE(16), height: data.readUInt32BE(20) };
}

describe('Bildmarke', () => {
  it('favicon.svg zeichnet Linie und Quadrat der Kopfleiste auf Kopf-Schwarz', () => {
    const svg = readFileSync(join(publicDir, 'favicon.svg'), 'utf8');
    expect(svg).toContain(
      `<path d="${LINE_PATH}" fill="none" stroke="${farbenDunkel.text}" stroke-width="${LINE_WIDTH}" stroke-linejoin="${LINE_JOIN}" stroke-miterlimit="${LINE_MITER_LIMIT}"/>`,
    );
    expect(svg).toContain(
      `<rect x="${SQUARE.x}" y="${SQUARE.y}" width="${SQUARE.size}" height="${SQUARE.size}" fill="${farbenDunkel.marke}"/>`,
    );
    expect(svg).toContain(`fill="${farbenDunkel.kopf}"`);
  });

  it('apple-touch-icon.png hat 180 px Kantenlänge', () => {
    expect(pngSize(join(publicDir, 'apple-touch-icon.png'))).toEqual({ width: 180, height: 180 });
  });

  it('index.html bindet Favicon und Symbol für iPad/iPhone ein', () => {
    const html = readFileSync(join(frontend, 'index.html'), 'utf8');
    expect(html).toContain('<link rel="icon" type="image/svg+xml" href="/favicon.svg" />');
    expect(html).toContain('<link rel="apple-touch-icon" href="/apple-touch-icon.png" />');
  });

  it('MARK_BOX umschließt die Spirale samt Strich (rechtwinklige Ecken, stumpfe Enden)', () => {
    const half = LINE_WIDTH / 2;
    const xs = [...LINE_PATH.matchAll(/[MH](-?\d+)/g)].map((m) => Number(m[1]));
    const ys = [0, ...[...LINE_PATH.matchAll(/V(-?\d+)/g)].map((m) => Number(m[1]))];
    // Jeder äußere Wert liegt auf einem Schenkel quer zur Richtung, dort steht der Strich halb über.
    expect(MARK_BOX).toEqual({
      x: Math.min(...xs) - half,
      y: Math.min(...ys) - half,
      width: Math.max(...xs) - Math.min(...xs) + LINE_WIDTH,
      height: Math.max(...ys) - Math.min(...ys) + LINE_WIDTH,
    });
  });

  it('ist Dekoration: für assistive Technik verborgen', () => {
    const { container } = render(<BrandMark height={22} lineColor="currentColor" />);
    const svg = container.querySelector('svg');
    expect(svg).toHaveAttribute('aria-hidden', 'true');
    expect(svg).toHaveAttribute('focusable', 'false');
  });
});
