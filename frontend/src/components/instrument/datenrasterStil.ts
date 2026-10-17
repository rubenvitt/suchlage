// Übernommen aus Lifeline Hub 3eb9115f2 (16.10.2026): frontend/src/components/instrument/datenrasterStil.ts
import type { CSSProperties } from 'react';
import type { Farbrollen } from '../../theme/tokens';
import { fugenrasterGrund } from './fugenraster';

/**
 * Stilfunktionen des Datenrasters — rein und exportiert (Muster `bedienzielStil`), damit
 * Fugenraster, Spaltendeckel und Mono-Wert ohne Render prüfbar sind. Eigene Datei mit
 * eigenem Basename: `datenraster.ts` neben `Datenraster.tsx` kollidierte case-insensitiv
 * (frontend/AGENTS.md, `direkteinstiegKern`).
 */

/** Mindestbreite einer Zelle, bevor das Raster eine Spalte abgibt. */
export const DATENRASTER_MINDESTBREITE = 180;

/**
 * Das Fugenraster (Grund und Fuge aus {@link fugenrasterGrund}), höchstens `spalten` Spalten.
 * Die Deckelung läuft über `max(Mindestbreite, Anteil)` in `auto-fill` — auf dem Handschirm
 * fällt das Raster von selbst auf eine Spalte, ohne Breitenfrage im Code (die gehörte sonst an
 * `useViewport`). Eine frei bleibende Spur zeigt `flaeche`, keine graue Kachel (LFH-961).
 */
export function datenrasterStil(
  rollen: Pick<Farbrollen, 'linie' | 'flaeche'>,
  spalten: number,
): CSSProperties {
  const n = Math.max(1, Math.floor(spalten));
  return {
    display: 'grid',
    gridTemplateColumns: `repeat(auto-fill, minmax(max(min(${DATENRASTER_MINDESTBREITE}px, 100%), calc((100% - ${n - 1}px) / ${n})), 1fr))`,
    margin: 0,
    ...fugenrasterGrund(rollen),
  };
}

/** Eine Zelle: Grund `flaeche`, Polster aus der Staffel. `breit` zieht über alle Spalten. */
export function datenfeldStil(
  rollen: Pick<Farbrollen, 'flaeche'>,
  token: { paddingSM: number; padding: number; marginXXS: number },
  breit: boolean,
): CSSProperties {
  return {
    display: 'flex',
    flexDirection: 'column',
    gap: token.marginXXS,
    minWidth: 0,
    paddingBlock: token.paddingSM,
    paddingInline: token.padding,
    background: rollen.flaeche,
    ...(breit ? { gridColumn: '1 / -1' } : {}),
  };
}
