// Übernommen aus Lifeline Hub 3eb9115f2 (16.10.2026): frontend/src/components/instrument/zielEinzug.ts
/**
 * Zielabstand im Fugenraster (Bedien-Leitlinie Kriterium 2, Tabelle „Abstand zwischen Zielen“):
 * zwei Bedienziele stehen in `komfortabel` ≥ 8 px und in `handschuh` ≥ 16 px auseinander,
 * `kompakt` hat die Spacing-Ausnahme (Fükw, Maus). Im Fugenraster lägen sie nur 1 px auseinander.
 *
 * Die Fuge bleibt deshalb 1 px, und das Ziel rückt INNERHALB seiner Rasterzelle um den Einzug
 * `e` vom Rand ab: zwischen zwei Treffflächen liegen Zellfläche, Fuge, Zellfläche, also
 * `2e + 1` px. Eingeführt am Kennzahlenband (LFH-630), nachgezogen für Segmentleiste und
 * Kartenknöpfe (LFH-865). Herleitung und verworfene Wege (breitere Fuge, Kacheln, verkleinerte
 * Trefffläche per `clip-path`):
 * `openspec/changes/archive/2026-10-01-lfh-630-kennzahlenband-handschuh-abstand/design.md`.
 *
 * Abgeleitet aus `controlHeight`, derselben Quelle wie Treffhöhe und Polsterung: unter einem
 * lokal überschriebenen Theme hielte die Zelle sonst die Höhe der einen und den Abstand der
 * anderen Stufe. Die Werte stehen als Literale: 2 · 4 + 1 ≥ 8 (komfortabel), 2 · 8 + 1 ≥ 16
 * (handschuh).
 */
export function zielEinzug(token: { controlHeight: number }): number {
  if (token.controlHeight >= 72) return 8;
  if (token.controlHeight >= 48) return 4;
  return 0;
}
