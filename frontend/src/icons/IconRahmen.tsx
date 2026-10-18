// Übernommen aus Lifeline Hub 5f07a20de (18.10.2026): frontend/src/icons/IconRahmen.tsx
/**
 * Rahmen jedes Icons des Satzes (LFH-595, Spec `iconsatz`).
 *
 * Die App zeichnet genau EINEN Iconsatz: Icons8 „iOS 27 Outlined“, für aktive Zustände der
 * Zwilling „iOS 27 Filled“. Suchlage erzeugt nichts selbst: `erzeugt.generated.ts` ist ein
 * Auszug aus Lifelines Register (`scripts/icons/` dort). Importiert wird nur über `icons/index.ts`.
 *
 * ── Warum der Rahmen sich wie ein antd-Icon verhält ──────────────────────────
 * Aufrufer tauschen nur den Import. Das Icon misst ohne `size` `1em` und wächst so mit der
 * Schrift, also mit der Dichte-Staffel 30/48/72. Die Hülle trägt die Klasse `anticon`: antd
 * richtet Icons in Knöpfen, Menüs, Tags und Eingaben über genau diese Klasse aus (Abstand zum
 * Text, Größe im Suffix). Ihre Grundausrichtung steht zusätzlich inline, weil antd die globale
 * `.anticon`-Regel erst mit seinem ersten eigenen Icon einspeist.
 *
 * ── Warum immer `aria-hidden` ──────────────────────────────────────────────────
 * Ein Icon trägt nie allein Bedeutung: den Namen trägt das Wort daneben oder der Knopf, der
 * nur ein Icon zeigt (`aria-label`). Deshalb gibt es keine Prop für einen Titel.
 */
import type { CSSProperties, JSX } from 'react';
import './icons.css';

export interface IconProps {
  /** Feste Kantenlänge (px oder CSS-Länge). Ohne Angabe `1em`: das Icon wächst mit der Schrift. */
  size?: number | string;
  /** Dreht das Icon (Ladeanzeige). Unter `prefers-reduced-motion: reduce` steht es still. */
  drehen?: boolean;
  className?: string;
  style?: CSSProperties;
}

/** Ein Icon des Satzes. Ersetzt `IconType` aus `react-icons`. */
export type Icon = ((props: IconProps) => JSX.Element | null) & { displayName?: string };

/** Umriss für inaktiv, Füllung für aktiv (Spec: „Aktiver Zustand über das gefüllte Icon“). */
export interface IconPaar {
  umriss: Icon;
  gefuellt: Icon;
}

/** Ein Pfad der Quelle. Icons8 zeichnet Konturen als Füllflächen, nicht als Strich. */
export interface Pfad {
  d: string;
  fillRule?: 'evenodd' | 'nonzero';
}

/** Grundausrichtung eines antd-Icons (`resetIcon` in `antd/es/style`). */
export const iconHuelleStil: CSSProperties = {
  display: 'inline-flex',
  alignItems: 'center',
  color: 'inherit',
  fontStyle: 'normal',
  lineHeight: 0,
  textAlign: 'center',
  textTransform: 'none',
  verticalAlign: '-0.125em',
};

/** Baut die Komponente zu einem Registereintrag. Nur `erzeugt.generated.tsx` ruft das auf. */
export function icon(name: string, viewBox: string, pfade: readonly Pfad[]): Icon {
  function IconKomponente({ size, drehen, className, style }: IconProps) {
    const kante = size ?? '1em';
    const klassen = ['anticon', 'lfh-icon', drehen ? 'lfh-icon-dreht' : null, className]
      .filter(Boolean)
      .join(' ');
    return (
      <span
        aria-hidden="true"
        data-lfh-icon={name}
        className={klassen}
        style={{ ...iconHuelleStil, ...style }}
      >
        <svg viewBox={viewBox} width={kante} height={kante} fill="currentColor" focusable="false">
          {pfade.map((p, i) => (
            <path key={i} d={p.d} fillRule={p.fillRule} />
          ))}
        </svg>
      </span>
    );
  }
  IconKomponente.displayName = `Icon(${name})`;
  return IconKomponente;
}
