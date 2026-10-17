// Übernommen aus Lifeline Hub 3eb9115f2 (16.10.2026): frontend/src/components/instrument/fugenraster.ts
import type { CSSProperties } from 'react';
import type { Farbrollen } from '../../theme/tokens';
// Die Fuge selbst steht als Regel `[data-fugenraster] > *` in der Gestaltungssprache.
import '../../theme/sprache.css';

/**
 * Das Fugenraster des Kennzahlenbands und des Datenrasters — rein und exportiert, damit beide
 * Bausteine und die Kopfleiste der Einsatzdaten denselben Grund tragen.
 *
 * DIE FUGE ZEICHNET DIE ZELLE (LFH-961). Früher lag `linie` als Grund unter dem ganzen Raster,
 * und `gap: 1px` ließ ihn als Haarlinie durchscheinen. Blieb eine Spur leer (fünf Kennzahlen
 * in zwei Spalten, vier Kopfangaben in drei, ein Datenraster mit mehr Spalten als Feldern),
 * schien er dort flächig durch: eine graue Kachel, die wie ein gesperrter oder nicht geladener
 * Wert aussah. Jetzt ist der Grund `flaeche`, und jede direkte Zelle trägt einen 1-px-Umriss in
 * `linie` (`sprache.css`, `[data-fugenraster] > *`). Benachbarte Umrisse decken sich in der
 * 1-px-Lücke zu genau einer Fuge, der äußere Umriss deckt sich mit dem Rahmen.
 *
 * WARUM `outline` UND NICHT `box-shadow`: die Kennzahl trägt ihre Eskalationskante als
 * Inline-`box-shadow` (`kennzahlStil`); ein Fugenschatten aus dem Stylesheet würde von ihr
 * überschrieben, einer im Inline-Stil müsste jede Zellart kennen. Der Umriss nimmt kein Layout,
 * also verschiebt er weder Zelle noch den Linkeinzug aus LFH-630 (`kennzahlZielEinzug`).
 *
 * `clipPath: inset(0)` schneidet am Rahmenrand ab: Wer den Rahmen abschaltet (Lage-Dashboard,
 * `border: 'none'`), bekommt keinen Umriss 1 px außerhalb des Bandes.
 *
 * Wer den Grund übernimmt, setzt am Raster auch `data-fugenraster`, sonst fehlen die Fugen.
 */
export function fugenrasterGrund(rollen: Pick<Farbrollen, 'linie' | 'flaeche'>): CSSProperties {
  return {
    gap: 1,
    background: rollen.flaeche,
    border: `1px solid ${rollen.linie}`,
    clipPath: 'inset(0)',
  };
}
