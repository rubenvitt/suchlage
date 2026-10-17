// Übernommen aus Lifeline Hub 3eb9115f2 (16.10.2026): frontend/src/components/instrument/fugenraster.test.ts
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import { farbenDunkel, farbenHell } from '../../theme/tokens';
import { fugenrasterGrund } from './fugenraster';

/**
 * LFH-961 — die Fuge zeichnet die Zelle, der Grund ist `flaeche`. Vitest fährt ohne CSS: hier
 * stehen der Grund und die Regel im Stylesheet; ob die Fuge wirklich 1 px zeigt und eine leere
 * Restspur Fläche bleibt, misst `e2e/fugenraster-leerzelle.spec.ts` am Pixel.
 */
const hier = dirname(fileURLToPath(import.meta.url));
const sprache = readFileSync(join(hier, '../../theme/sprache.css'), 'utf-8');

describe('fugenrasterGrund', () => {
  it.each([
    ['hell', farbenHell],
    ['dunkel', farbenDunkel],
  ])('%s: Grund flaeche, Rahmen linie, Lücke 1 px', (_, rollen) => {
    const stil = fugenrasterGrund(rollen);
    expect(stil).toMatchObject({
      gap: 1,
      background: rollen.flaeche,
      border: `1px solid ${rollen.linie}`,
    });
    expect(stil.background).not.toBe(rollen.linie);
  });

  it('schneidet am Rahmenrand ab, damit ein Band ohne Rahmen keinen Außenumriss trägt', () => {
    expect(fugenrasterGrund(farbenHell).clipPath).toBe('inset(0)');
  });

  it('die Zelle trägt die Fuge: 1-px-Umriss in linie an jedem direkten Kind', () => {
    const regel = /\[data-fugenraster\]\s*>\s*\*\s*\{([^}]*)\}/.exec(sprache);
    expect(regel, 'Regel `[data-fugenraster] > *` fehlt in sprache.css').not.toBeNull();
    expect(regel![1]).toMatch(/outline:\s*1px solid var\(--lfh-linie\);/);
  });
});
