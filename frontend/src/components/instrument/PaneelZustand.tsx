// Übernommen aus Lifeline Hub 3eb9115f2 (16.10.2026): frontend/src/components/instrument/PaneelZustand.tsx
import { Button, Skeleton } from 'antd';
import type { ReactNode } from 'react';
import { useRollen } from './rollenwerte';

/**
 * Datenzustand eines Paneel-Körpers. Dieselben vier Werte wie `Datenzustand` in
 * `pages/lage-dashboard/lagebild.ts` — der Baustein führt sie SELBST, damit er nicht von
 * einer Seite abhängt. Wer dort umstellt, tauscht den Typ ohne Wertänderung.
 */
export type PaneelDatenzustand = 'daten' | 'laden' | 'fehler' | 'leer';

/** Wortlaut des Fehlerzweigs — exportiert, damit Tests ihn nicht nachschreiben müssen. */
export const PANEEL_FEHLER_TITEL = 'Daten nicht abrufbar';
export const PANEEL_FEHLER_TEXT =
  'Stand unbekannt — nicht als Lage melden. Letzter Abruf fehlgeschlagen.';
export const PANEEL_NEULADEN = 'Erneut abrufen';
/** Satz des Ladezustands — derselbe Wortlaut wie in `Kennzahl`. */
export const PANEEL_LADEN_TEXT = 'wird abgerufen';

interface PaneelZustandProps {
  zustand: PaneelDatenzustand;
  /** Paneeltitel — benennt den Ladezustand für Vorlesende („Meldungen wird geladen"). */
  titel: string;
  leerText: string;
  /**
   * Beschriftung der Aktion im Leerzustand („Meldung erfassen"). Fehlt sie, steht nur der Satz
   * da: richtig, wenn es keinen Ausweg gibt (ohne Schreibrecht führte eine Aktion ins Leere) ODER
   * wenn der Ausweg anderswo steht, etwa im Paneelkopf — zwei gleichnamige Ziele mit derselben
   * Wirkung wären für Vorlesende nicht unterscheidbar. Für jemanden, der schreiben darf, bleibt
   * ein Leerzustand ohne jeden Ausweg eine Sackgasse.
   */
  leerAktion?: string;
  onLeerAktion?: () => void;
  onNeuladen: () => void;
  children: ReactNode;
}

/**
 * PaneelZustand — Laden · Fehler · Leer im Körper eines `Paneel`. Drei Zustände, drei
 * Erscheinungen (LFH-331 · B3): sähe ein Fehler aus wie leer, meldete jemand am Funkgerät eine
 * falsche Lage.
 *
 * - `laden` → „wird abgerufen“ mit `aria-busy` und benanntem Ladezustand, in der Form des
 *   Leerzustands (der Platz der Aktion als Skelett in Knopfhöhe, LFH-629),
 * - `fehler` → `role="alert"`, Titel in `alarm`, „Stand unbekannt", Knopf „Erneut abrufen",
 * - `leer` → gedämpfter Satz plus eine AKTION (ein Leerzustand ohne Ausweg ist eine Sackgasse),
 * - `daten` → die Kinder, unverändert.
 *
 * Die Knöpfe sind antd-`Button` und erben `controlHeight` vom `ConfigProvider`.
 */
export default function PaneelZustand({
  zustand,
  titel,
  leerText,
  leerAktion,
  onLeerAktion,
  onNeuladen,
  children,
}: PaneelZustandProps) {
  const { token, rollen } = useRollen();
  const polster = { padding: token.padding } as const;

  if (zustand === 'laden') {
    // Dieselbe Form wie der Leerzustand (LFH-629): Satzzeile und, wo der Leerzustand eine Aktion
    // trägt, ihr Platz in Knopfhöhe. Das dreizeilige Skelett davor war auf dem Handschirm 50 px
    // höher als „leer“; untereinander gestapelt addierten sich die Differenzen, und das letzte
    // Paneel wanderte 150 px. Mit Daten wächst das Paneel weiter — nach unten, wo es nur schiebt,
    // was unter ihm steht.
    return (
      <div
        aria-busy="true"
        aria-label={`${titel} wird geladen`}
        style={{ ...polster, display: 'flex', flexDirection: 'column', gap: token.marginXS }}
      >
        <span style={{ color: rollen.gedaempft, fontSize: 12 }}>{PANEEL_LADEN_TEXT}</span>
        {leerAktion != null && onLeerAktion != null && (
          <span>
            <Skeleton.Button active />
          </span>
        )}
      </div>
    );
  }
  if (zustand === 'fehler') {
    return (
      <div
        role="alert"
        data-lfh="paneel-fehler"
        style={{ ...polster, display: 'flex', flexDirection: 'column', gap: token.marginXS }}
      >
        <b style={{ color: rollen.alarmText, fontSize: 13 }}>{PANEEL_FEHLER_TITEL}</b>
        <span style={{ color: rollen.gedaempft, fontSize: 12 }}>{PANEEL_FEHLER_TEXT}</span>
        <span>
          <Button onClick={onNeuladen}>{PANEEL_NEULADEN}</Button>
        </span>
      </div>
    );
  }
  if (zustand === 'leer') {
    return (
      <div
        data-lfh="paneel-leer"
        style={{ ...polster, display: 'flex', flexDirection: 'column', gap: token.marginXS }}
      >
        <span style={{ color: rollen.gedaempft, fontSize: 12 }}>{leerText}</span>
        {leerAktion != null && onLeerAktion != null && (
          <span>
            <Button onClick={onLeerAktion}>{leerAktion}</Button>
          </span>
        )}
      </div>
    );
  }
  return <>{children}</>;
}

/**
 * Link-Knopf „… ↗" für den Paneelkopf (`Paneel aktion`). Der Pfeil ist Zeichen und
 * `aria-hidden`: der zugängliche Name ist das Wort.
 */
export function PaneelLink({ label, onKlick }: { label: string; onKlick: () => void }) {
  const { token } = useRollen();
  return (
    <Button
      type="link"
      data-lfh="paneel-link"
      onClick={onKlick}
      style={{ paddingInline: token.paddingXS, fontSize: 12 }}
    >
      {label}
      <span aria-hidden="true" style={{ marginInlineStart: token.marginXXS }}>
        ↗
      </span>
    </Button>
  );
}
