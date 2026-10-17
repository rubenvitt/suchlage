// Übernommen aus Lifeline Hub 3eb9115f2 (16.10.2026): frontend/src/components/instrument/statusFlaeche.ts (Statusrolle aus theme/status)
import type { Statusrolle } from '../../theme/status';
import type { Farbrollen } from '../../theme/tokens';

/**
 * Status als getönte FLÄCHE mit getöntem Text — „Ampel als Fläche, Zahl bleibt lesbar". Die eine
 * Übersetzung Ton → Grund/Text/Kante für `StatusZelle`, `StatusChip` und die
 * Flächen-Darstellung von `StatusTag`.
 *
 * ── DIE KONTRASTRECHNUNG ────────────────────────────────────────────────────────────
 *
 * Böden: Kriterium 5 der Bedien-Leitlinie, gemessen in `e2e/betroffene-kontrast.spec.ts` und
 * `e2e/kraefte-kontrast.spec.ts` — Tag ≥ 7 : 1, Nacht ≥ 5 : 1. WCAG-Formel aus
 * `farbenHell`/`farbenDunkel`:
 *
 * | Ton      | Text / Grund                 | Tag   | Nacht |
 * |----------|------------------------------|-------|-------|
 * | normal   | normalText / normalFlaeche   | 7,87  | 10,44 |
 * | bedien   | bedienText / bedienFlaeche   | 8,04  |  9,65 |
 * | achtung  | achtungText / achtungFlaeche | 8,02  | 11,18 |
 * | alarm    | alarmText / alarmFlaeche     | 7,31  |  6,89 |
 * | neutral  | text2 / flaeche3             | 10,30 | 10,89 |
 *
 * Die Füllfarben von `achtung`/`alarm` tragen als Text den Tagesboden nicht (6,02 bzw. 5,52),
 * deshalb die Textrollen `achtungText`/`alarmText`; die Füllfarbe bleibt die KANTE.
 *
 * `neutral` hat keine Statusfläche — `flaeche3` + `text2`. Seit LFH-643 hielten auch `gedaempft`
 * und `schwach` dort den Boden (Tag 8,62 / 7,05, Nacht 6,83 / 5,11); `text2` bleibt, damit das
 * neutrale Zustandswort nachts nicht schwächer wirkt als die übrigen (alle ≥ 6,89).
 *
 * `kante` ist die Rollenfarbe für einen Rahmen, der sich vom Grund abhebt (WCAG 1.4.11,
 * ≥ 3 : 1 — `kraefte-kontrast.spec.ts` prüft ihn am `StatusTag`): Tag normal 5,96 · bedien
 * 5,57 · achtung 6,02 · alarm 5,52 · neutral (schwach/flaeche3) 7,05; Nacht 7,93 · 5,66 ·
 * 11,18 · 6,89 · 5,11.
 */
export type StatusTon = 'normal' | 'achtung' | 'alarm' | 'bedien' | 'neutral';

interface StatusFlaecheWerte {
  grund: string;
  text: string;
  kante: string;
}

type Benoetigt = Pick<
  Farbrollen,
  | 'normal'
  | 'normalText'
  | 'normalFlaeche'
  | 'achtung'
  | 'achtungText'
  | 'achtungFlaeche'
  | 'alarm'
  | 'alarmText'
  | 'alarmFlaeche'
  | 'bedien'
  | 'bedienText'
  | 'bedienFlaeche'
  | 'flaeche3'
  | 'text2'
  | 'schwach'
>;

export function statusFlaeche(rollen: Benoetigt, ton: StatusTon): StatusFlaecheWerte {
  switch (ton) {
    case 'normal':
      return { grund: rollen.normalFlaeche, text: rollen.normalText, kante: rollen.normal };
    case 'bedien':
      return { grund: rollen.bedienFlaeche, text: rollen.bedienText, kante: rollen.bedien };
    case 'achtung':
      return { grund: rollen.achtungFlaeche, text: rollen.achtungText, kante: rollen.achtung };
    case 'alarm':
      return { grund: rollen.alarmFlaeche, text: rollen.alarmText, kante: rollen.alarm };
    case 'neutral':
      return { grund: rollen.flaeche3, text: rollen.text2, kante: rollen.schwach };
  }
}

/**
 * Welche Statusrolle hat eine Fläche? `marke` nicht: sie ist Signatur (Logo, Rail-Marke),
 * kein Zustand, und hat keine Statusfläche. Rein.
 */
export function tonVonRolle(rolle: Statusrolle): StatusTon | null {
  return rolle === 'marke' ? null : rolle;
}
