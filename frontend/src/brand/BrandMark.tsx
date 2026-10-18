/**
 * Bildmarke „Suchspirale“: das Zeichen von Suchlage in der Oberfläche.
 *
 * Geometrie aus `brandMarkGeometry.ts`, dieselbe wie im Favicon. Die Linie nimmt die Textfarbe
 * ihres Grundes von außen, das Quadrat ist `marke`, Rot bleibt Akzent.
 *
 * Reine Dekoration: `aria-hidden`, nicht fokussierbar. Den zugänglichen Namen trägt die Wortmarke
 * daneben.
 */
import { farbenDunkel } from '../theme/tokens';
import {
  LINE_JOIN,
  LINE_MITER_LIMIT,
  LINE_PATH,
  LINE_WIDTH,
  MARK_BOX,
  MARK_VIEWBOX,
  SQUARE,
} from './brandMarkGeometry';

export function BrandMark({ height, lineColor }: { height: number; lineColor: string }) {
  return (
    <svg
      data-sl="brand-mark"
      aria-hidden="true"
      focusable="false"
      viewBox={MARK_VIEWBOX}
      height={height}
      width={(height * MARK_BOX.width) / MARK_BOX.height}
      style={{ display: 'block', flex: 'none' }}
    >
      <path
        d={LINE_PATH}
        fill="none"
        stroke={lineColor}
        strokeWidth={LINE_WIDTH}
        strokeLinejoin={LINE_JOIN}
        strokeMiterlimit={LINE_MITER_LIMIT}
      />
      <rect x={SQUARE.x} y={SQUARE.y} width={SQUARE.size} height={SQUARE.size} fill={farbenDunkel.marke} />
    </svg>
  );
}
