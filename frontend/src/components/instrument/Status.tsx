// Übernommen aus Lifeline Hub 3eb9115f2 (16.10.2026): frontend/src/components/instrument/Status.tsx
import type { CSSProperties, ReactNode } from 'react';
import { monoStil, useRollen } from './rollenwerte';
import { statusFlaeche, type StatusTon } from './statusFlaeche';

/**
 * StatusZelle und StatusChip — Status als getönte Fläche (Neuentwurf S1 „Statuszelle",
 * S2 Abschnittsraster, S6 Einheitenliste). Die Farbwerte und ihre Kontrastrechnung stehen
 * in {@link statusFlaeche}; hier wird nur angeordnet.
 *
 * DER ZWEITE KANAL IST PFLICHT, und zwar im Typ: `wort` ist ein Pflicht-Prop. Die Zelle
 * zeigt oft nur eine Zahl (die Legende über dem Raster bzw. im Spaltenkopf nennt die Bedeutung
 * fürs Auge); deshalb trägt sie das Wort unsichtbar hinter der Zahl und als `title` — ein
 * Vorleser hört „2 bereit", nicht „2".
 *
 * KEIN Bedienziel: beide sind Anzeige. Wer eine Zelle klickbar braucht, legt einen Link
 * oder Knopf DARUM (mit den zwei Angaben aus LFH-365), statt hier einen `onClick` zu
 * erwarten.
 */

/** Visuell verborgen, für Vorleser da — die übliche Clip-Bauform. */
export const NUR_VORLESER: CSSProperties = {
  position: 'absolute',
  width: 1,
  height: 1,
  padding: 0,
  margin: -1,
  overflow: 'hidden',
  clip: 'rect(0, 0, 0, 0)',
  whiteSpace: 'nowrap',
  border: 0,
};

interface StatusZelleProps {
  ton: StatusTon;
  /** Der angezeigte Wert, meist eine Zahl. */
  wert: ReactNode;
  /** Zweiter Kanal — Pflicht. */
  wort: string;
  /** Zellenhöhe in px; Vorgabe 34 (Entwurf S1), 24 für dichte Raster (S2). */
  hoehe?: number;
  style?: CSSProperties;
}

/** Raster-taugliche Zelle: steht im Fugenraster (`gap: 1px` auf `linie`) des Aufrufers. */
export function StatusZelle({ ton, wert, wort, hoehe = 34, style }: StatusZelleProps) {
  const { rollen } = useRollen();
  const f = statusFlaeche(rollen, ton);
  return (
    <div
      data-lfh="status-zelle"
      data-ton={ton}
      title={wort}
      style={{
        position: 'relative',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        minHeight: hoehe,
        background: f.grund,
        color: f.text,
        ...monoStil(hoehe >= 30 ? 13 : 12),
        ...style,
      }}
    >
      {wert}
      <span style={NUR_VORLESER}> {wort}</span>
    </div>
  );
}

interface StatusChipProps {
  ton: StatusTon;
  /** Zweiter Kanal — Pflicht (Wort neben dem Code). */
  wort: string;
  /** Optionaler Code vor dem Wort (FMS „2", „SK III"), Mono 11/500. */
  code?: ReactNode;
  /**
   * Sichtbarer Punkt zwischen Code und Wort, das FMS-Format „S4 · Am Einsatzort“ (LFH-973).
   * Nur für einen Status-Code: eine Zahl als Code („2 ausstehend“) bleibt ohne Punkt.
   */
  trenner?: boolean;
  title?: string;
  style?: CSSProperties;
}

/** Inline-Chip, Höhe 22 (Entwurf S6): Fläche + Code Mono + Wort 11. */
export function StatusChip({ ton, wort, code, trenner, title, style }: StatusChipProps) {
  const { token, rollen } = useRollen();
  const f = statusFlaeche(rollen, ton);
  return (
    <span
      data-lfh="status-chip"
      data-ton={ton}
      title={title}
      style={{
        display: 'inline-flex',
        alignItems: 'center',
        gap: token.marginXS,
        height: 22,
        paddingInline: 8,
        background: f.grund,
        color: f.text,
        whiteSpace: 'nowrap',
        ...style,
      }}
    >
      {code != null && <span style={monoStil(11, 500)}>{code}</span>}
      {/* Vorgelesen trennt der Abstand, der Punkt ist Optik. */}
      {code != null && trenner && <span aria-hidden="true">·</span>}
      <span style={{ fontSize: 11 }}>{wort}</span>
    </span>
  );
}
