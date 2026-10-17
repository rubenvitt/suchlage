// Übernommen aus Lifeline Hub 3eb9115f2 (16.10.2026): frontend/src/components/instrument/Aufgliederung.tsx
import type { CSSProperties, ReactNode } from 'react';
import { monoStil, useRollen } from './rollenwerte';

/**
 * Aufgliederung und Balken — die zwei Mengenbilder des Neuentwurfs.
 *
 * **Aufgliederung** (S1 „Kennzahl-Kachel", S2): ein gestapelter Balken, Segmente per
 * `flex` nach ihrem Wert, 2-px-Fugen, darunter eine Mono-Legende. „Zahl führt,
 * Aufgliederung folgt" — der Balken ersetzt nie die Zahl.
 *
 * **Balken** (S4 Tagesbilanz): Anteil einer Menge. Spur
 * `flaeche3`, Füllung in der übergebenen Farbe.
 *
 * FARBEN KOMMEN VOM AUFRUFER, bereits aufgelöst (`rollenFarbe`, `sichtungsfarben`,
 * `warnstufeBalkenFarbe` …). Der Baustein erfindet keine Zuordnung — sonst stünde hier eine
 * zweite Statuskarte neben `theme/statusFarben.ts`.
 *
 * ZUGÄNGLICH als EIN Bild (`role="img"`) mit ausgeschriebenem Wortlaut: ein Vorleser soll
 * „Betroffene nach Sichtung: SK I 12, SK II 31 …" hören, nicht n namenlose Kästchen. Die
 * Legende daneben ist deshalb `aria-hidden` — sie sagt dasselbe noch einmal fürs Auge.
 * Der zweite Kanal (WCAG 1.4.1) ist die Legende mit Wortlaut und Zahl, nicht die Farbe.
 */

export interface Segment {
  /** Wortlaut — Pflicht, er IST der zweite Kanal. */
  label: string;
  wert: number;
  /** Aufgelöste Farbe (Rolle/Fachfarbe des aktiven Modus). */
  farbe: string;
  /**
   * Optionale Umrandung des Segments (aufgelöste Farbe). Für Fachfarben, die auf einem Grund
   * verschwinden: das schwarze „tot" auf dem Nachtgrund, Gelb (SK II) auf hellem — dieselbe
   * Begründung wie am Farbfeld des `SichtungsTag`. Als `outline` mit Versatz −1 px gezeichnet,
   * also OHNE Layoutwirkung: der Anteil des Segments bleibt exakt sein Wert.
   */
  umrandung?: string;
}

/** Stil eines Segments — rein und exportiert, damit die Umrandung ohne Render prüfbar ist. */
export function segmentFlaecheStil(s: Segment): CSSProperties {
  return {
    flexGrow: s.wert,
    flexShrink: 1,
    flexBasis: 0,
    background: s.farbe,
    ...(s.umrandung != null ? { outline: `1px solid ${s.umrandung}`, outlineOffset: -1 } : {}),
  };
}

/** Wortlaut für `aria-label`: „Titel: A 1, B 2". Rein — ohne Render prüfbar. */
export function aufgliederungText(segmente: readonly Segment[], titel?: string): string {
  const teile = segmente.map((s) => `${s.label} ${s.wert}`).join(', ');
  return titel ? `${titel}: ${teile}` : teile;
}

/** Nur positive Werte bekommen ein Segment — ein 0-Segment wäre eine Fuge ohne Fläche. */
export function sichtbareSegmente(segmente: readonly Segment[]): Segment[] {
  return segmente.filter((s) => Number.isFinite(s.wert) && s.wert > 0);
}

interface AufgliederungProps {
  segmente: readonly Segment[];
  /** Voranstellung im zugänglichen Namen („Betroffene nach Sichtung"). */
  titel?: string;
  /** Balkenhöhe 5–8 px; Vorgabe 5. */
  hoehe?: 5 | 6 | 7 | 8;
  /** Eigene Legende statt der automatischen „SK I 12 · SK II 31". `false` blendet sie aus. */
  legende?: ReactNode | false;
  style?: CSSProperties;
}

export function Aufgliederung({ segmente, titel, hoehe = 5, legende, style }: AufgliederungProps) {
  const { token, rollen } = useRollen();
  const sichtbar = sichtbareSegmente(segmente);
  const legendenInhalt =
    legende === false ? null : (legende ?? segmente.map((s) => `${s.label} ${s.wert}`).join(' · '));
  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: token.marginXS, ...style }}>
      <div
        role="img"
        aria-label={aufgliederungText(segmente, titel)}
        data-lfh="aufgliederung"
        style={{
          display: 'flex',
          gap: 2,
          height: hoehe,
          // Leere Menge: die Spur steht trotzdem — ein verschwindender Balken sähe aus wie
          // „nicht geladen", nicht wie „null".
          background: sichtbar.length === 0 ? rollen.flaeche3 : undefined,
        }}
      >
        {sichtbar.map((s) => (
          <span key={s.label} data-segment={s.label} style={segmentFlaecheStil(s)} />
        ))}
      </div>
      {legendenInhalt != null && (
        <div aria-hidden="true" style={{ ...monoStil(11), color: rollen.gedaempft }}>
          {legendenInhalt}
        </div>
      )}
    </div>
  );
}

/** Anteil 0…1, sicher gegen Division durch null und Überlauf. Rein. */
export function anteil(wert: number, max: number): number {
  if (!Number.isFinite(wert) || !Number.isFinite(max) || max <= 0) return 0;
  return Math.min(1, Math.max(0, wert / max));
}

interface BalkenProps {
  wert: number;
  /** Bezugsgröße; Vorgabe 1 (dann ist `wert` bereits ein Anteil). */
  max?: number;
  /** Aufgelöste Füllfarbe. */
  farbe: string;
  /** Zugänglicher Wortlaut — Pflicht („Abschnitt Nord: 60 Prozent"). */
  beschriftung: string;
  /** Sichtbares Mono-Label rechts neben der Spur („60 %"). */
  label?: ReactNode;
  /** Spurhöhe 4–6 px; Vorgabe 4. */
  hoehe?: 4 | 5 | 6;
  style?: CSSProperties;
}

export function Balken({
  wert,
  max = 1,
  farbe,
  beschriftung,
  label,
  hoehe = 4,
  style,
}: BalkenProps) {
  const { token, rollen } = useRollen();
  const breite = `${anteil(wert, max) * 100}%`;
  return (
    <div style={{ display: 'flex', alignItems: 'center', gap: token.marginXS, ...style }}>
      <div
        role="img"
        aria-label={beschriftung}
        data-lfh="balken"
        style={{ flex: '1 1 auto', height: hoehe, background: rollen.flaeche3, display: 'flex' }}
      >
        <span style={{ width: breite, background: farbe }} />
      </div>
      {label != null && (
        <span aria-hidden="true" style={{ ...monoStil(11), color: rollen.schwach }}>
          {label}
        </span>
      )}
    </div>
  );
}
