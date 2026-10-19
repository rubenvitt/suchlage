// Übernommen aus Lifeline Hub 3eb9115f2 (16.10.2026): frontend/src/components/instrument/Segmentleiste.tsx
import { Tooltip } from 'antd';
import { useId, useRef, type CSSProperties, type KeyboardEvent } from 'react';
import { useRollen } from './rollenwerte';
import { zielEinzug } from './zielEinzug';
import '../../theme/sprache.css';

/**
 * Segmentleiste — Umschalter im Instrumentenstil (ETB-Filter, Basiskarte, Ansicht). Ersatz für
 * antds `Segmented`, dessen Pillenform, Schatten und Gleitanimation die Formensprache (Radius 0,
 * Fugenraster) brechen.
 *
 * FUGENRASTER: die Segmente stehen mit `gap: 1px` auf `linieStark`; aktiv `flaeche3` + `text`,
 * inaktiv `flaeche` + `gedaempft`, optional ein 6-px-Farbpunkt (quadratisch). Farben und
 * Hover/Fokus liegen als Klassen in `theme/sprache.css` (`.lfh-segmente`, `.lfh-segment`), weil
 * `:hover` und `:focus-visible` inline nicht erreichbar sind; die GEOMETRIE steht inline aus den
 * Tokens.
 *
 * ZUGÄNGLICH als `radiogroup` (Vorgabe, eine Wahl unter mehreren) oder als `tablist` (wenn das
 * Segment eine Fläche darunter umschaltet). Tastatur nach APG: ←/→ (und ↑/↓) wandern und
 * wählen, Pos1/Ende springen; nur das gewählte Segment liegt in der Tab-Reihenfolge (roving
 * tabindex).
 *
 * BEDIENZIEL: jedes Segment ist ein `<button>` ohne antd-Höhe und trägt die ZWEI Angaben aus
 * LFH-365 ({@link segmentStil}). Die Staffel 30 / 48 / 72 gilt, nicht die Entwurfsskizze.
 *
 * ZIELABSTAND (LFH-865, Muster LFH-630, Bedien-Leitlinie Kriterium 2): die Fuge bleibt 1 px,
 * und jedes Segment steht in einer eigenen Rasterzelle (`data-lfh="segment-zelle"`, Grund
 * `flaeche`), in der es um {@link zielEinzug} (0 / 4 / 8 px) von jedem Rand abrückt — auch oben
 * und unten, weil die Leiste umbricht. Benachbarte Segmente stehen so in `komfortabel` ≥ 8 px,
 * in `handschuh` ≥ 16 px auseinander. Die waagerechte Polsterung des Segments sinkt um den
 * Einzug, die Breite bleibt; die Leiste wird um 2 × Einzug höher, die Treffhöhe bleibt
 * `controlHeight`. Die Zelle hat `role="none"`: Radiogruppe und Tabliste besitzen ihre Segmente
 * weiter unmittelbar, Pfeiltasten und roving tabindex bleiben am `<button>`. Aktiv-Fläche,
 * Hover und Fokus liegen auf dem Segment, nicht auf der Zelle: sie zeigen, wo ein Tippen wirkt,
 * der Rand bleibt still (wie die Kennzahl, LFH-630 D4).
 *
 * GESPERRT: ein Segment mit `gesperrt` bleibt SICHTBAR und nennt seinen Grund — als Tooltip und
 * als Beschreibung (`aria-describedby`). Es ist `aria-disabled`, nicht `disabled`: ein natives
 * `disabled` nähme es aus Fokus und Hilfstechnik, der Grund wäre unerreichbar. Klick und Pfeile
 * wählen es nicht, die Pfeile springen darüber hinweg. Der Grund ist der zweite Kanal neben
 * dem Grau (WCAG 1.4.1).
 */

export interface SegmentOption<W extends string | number> {
  wert: W;
  /** Sichtbarer Wortlaut — Pflicht, auch neben einem Farbpunkt. */
  label: string;
  /** Aufgelöste Farbe für den 6-px-Punkt (z. B. `etbTypFarbe(typ, token).kante`). */
  punkt?: string;
  /** Nur `tablist`: Id der Fläche, die dieses Segment zeigt. */
  steuert?: string;
  /**
   * Sperrt das Segment und nennt den GRUND („Keine Offline-Karte hinterlegt"). Ohne Grund keine
   * Sperre — ein stumm gesperrtes Segment ist von „kaputt" nicht zu unterscheiden.
   */
  gesperrt?: string;
}

/**
 * Geometrie eines Segments — rein und exportiert (Muster `bedienzielStil`). Die waagerechte
 * Polsterung ist `paddingSM` abzüglich des Einzugs der Zelle ({@link segmentZelleStil}): Zelle
 * und Segment zusammen sind so breit wie ein Segment ohne Einzug.
 */
export function segmentStil(token: {
  controlHeight: number;
  paddingSM: number;
  marginXS: number;
}): CSSProperties {
  return {
    display: 'inline-flex',
    alignItems: 'center',
    gap: token.marginXS,
    minHeight: token.controlHeight,
    paddingInline: token.paddingSM - zielEinzug(token),
    paddingBlock: 0,
  };
}

/**
 * Rasterzelle eines Segments — rein und exportiert. Der Einzug ringsum hält den Zielabstand
 * (Dateikopf „Zielabstand“); den Grund trägt die Klasse `.lfh-segment-zelle` (`sprache.css`).
 */
export function segmentZelleStil(token: { controlHeight: number }): CSSProperties {
  return { display: 'flex', padding: zielEinzug(token) };
}

/**
 * Der Index nach einer Pfeil-/Pos1-/Ende-Taste, oder `null`, wenn die Taste nicht wandert.
 *
 * `gesperrt(i)` überspringt Segmente: Pfeile laufen zum nächsten freien (zyklisch), Pos1/Ende
 * zum ersten/letzten freien. Ist keines frei außer dem aktuellen, bleibt es beim aktuellen;
 * ist gar keines frei, wandert nichts (`null`).
 */
export function naechsterIndex(
  taste: string,
  aktuell: number,
  anzahl: number,
  gesperrt: (index: number) => boolean = () => false,
): number | null {
  if (anzahl === 0) return null;
  const frei = (i: number) => !gesperrt(i);
  const suche = (start: number, schritt: 1 | -1): number | null => {
    for (let n = 0; n < anzahl; n += 1) {
      const i = (((start + schritt * n) % anzahl) + anzahl) % anzahl;
      if (frei(i)) return i;
    }
    return null;
  };
  switch (taste) {
    case 'ArrowRight':
    case 'ArrowDown':
      return suche(aktuell + 1, 1);
    case 'ArrowLeft':
    case 'ArrowUp':
      return suche(aktuell - 1, -1);
    case 'Home':
      return suche(0, 1);
    case 'End':
      return suche(anzahl - 1, -1);
    default:
      return null;
  }
}

interface SegmentleisteProps<W extends string | number> {
  optionen: readonly SegmentOption<W>[];
  wert: W;
  onWechsel: (wert: W) => void;
  /** Zugänglicher Name der Gruppe — Pflicht („Einträge nach Typ filtern"). */
  beschriftung: string;
  rolle?: 'radiogroup' | 'tablist';
  style?: CSSProperties;
}

export default function Segmentleiste<W extends string | number>({
  optionen,
  wert,
  onWechsel,
  beschriftung,
  rolle = 'radiogroup',
  style,
}: SegmentleisteProps<W>) {
  const { token } = useRollen();
  const grundId = useId();
  const knoepfe = useRef<(HTMLButtonElement | null)[]>([]);
  const aktivIndex = Math.max(
    0,
    optionen.findIndex((o) => o.wert === wert),
  );
  const alsTab = rolle === 'tablist';

  const taste = (e: KeyboardEvent<HTMLButtonElement>, index: number) => {
    const ziel = naechsterIndex(e.key, index, optionen.length, (i) => !!optionen[i].gesperrt);
    if (ziel == null) return;
    e.preventDefault();
    onWechsel(optionen[ziel].wert);
    knoepfe.current[ziel]?.focus();
  };

  return (
    <div
      role={rolle}
      aria-label={beschriftung}
      className="lfh-segmente"
      data-lfh="segmentleiste"
      style={style}
    >
      {optionen.map((o, i) => {
        const aktiv = i === aktivIndex;
        const gesperrt = o.gesperrt != null && o.gesperrt !== '';
        const beschreibungId = gesperrt ? `${grundId}-${i}` : undefined;
        const knopf = (
          <button
            ref={(el) => {
              knoepfe.current[i] = el;
            }}
            type="button"
            role={alsTab ? 'tab' : 'radio'}
            aria-checked={alsTab ? undefined : aktiv}
            aria-selected={alsTab ? aktiv : undefined}
            aria-controls={alsTab ? o.steuert : undefined}
            aria-disabled={gesperrt || undefined}
            aria-describedby={beschreibungId}
            data-gesperrt={gesperrt || undefined}
            tabIndex={aktiv ? 0 : -1}
            className={aktiv ? 'lfh-segment lfh-segment--aktiv' : 'lfh-segment'}
            onClick={() => {
              if (!gesperrt) onWechsel(o.wert);
            }}
            onKeyDown={(e) => taste(e, i)}
            style={segmentStil(token)}
          >
            {o.punkt != null && (
              <span
                aria-hidden="true"
                data-lfh="segment-punkt"
                style={{ width: 6, height: 6, flex: '0 0 6px', background: o.punkt }}
              />
            )}
            {o.label}
            {gesperrt && (
              // Der Grund als Beschreibung — sichtbar trägt ihn der Tooltip.
              <span id={beschreibungId} hidden>
                {o.gesperrt}
              </span>
            )}
          </button>
        );
        return (
          <span
            key={String(o.wert)}
            role="none"
            className="lfh-segment-zelle"
            data-lfh="segment-zelle"
            style={segmentZelleStil(token)}
          >
            {gesperrt ? <Tooltip title={o.gesperrt}>{knopf}</Tooltip> : knopf}
          </span>
        );
      })}
    </div>
  );
}
