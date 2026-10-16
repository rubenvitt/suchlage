// Übernommen aus Lifeline Hub 3eb9115f2 (16.10.2026): frontend/src/components/instrument/Paneel.tsx
import { useId, type CSSProperties, type ReactNode } from 'react';
import type { Farbrollen } from '../../theme/tokens';
import Augenbraue from './Augenbraue';
import { monoStil, useRollen } from './rollenwerte';

/**
 * Paneel — die Grundfläche für Seitenleisten, Kachel-Paneele und Listenblöcke.
 *
 * Rahmen `linie`, Grund `paneel`, Kopf 38 px mit Augenbraue links und optionalem Mono-Meta bzw.
 * einer Aktion rechts, Haarlinie darunter, Radius 0. Der Körper trägt keine eigene Polsterung —
 * Zeilen bringen sie mit (Trenner `flaeche3`, siehe {@link paneelZeileStil}); wer Fließinhalt
 * hineinlegt, setzt `koerperPolster`.
 *
 * Die Überschrift ist SEMANTISCH (`ueberschrift="h2"` … `"h6"`), die Augenbraue nur ihre Optik;
 * das `<section>` wird über sie benannt (`aria-labelledby`). Die Ebene folgt der Gliederung:
 * ein Paneel in einem Paneel ist eine Ebene tiefer, sieht aber gleich aus. Ein Paneel ohne
 * Überschrift gibt es nicht.
 *
 * Die 38 px sind eine MINDESThöhe: steht rechts eine Aktion, wächst der Kopf mit deren
 * Dichte-Staffel mit.
 *
 * ENGER KOPF: der Kopf BRICHT UM (`flexWrap`), der rechte Block rückt als Ganzes in eine zweite
 * Zeile und bleibt rechtsbündig (`marginInlineStart: auto`); reicht selbst die nicht, kürzt das
 * Meta mit Auslassung (`paneelMetaStil`), die Aktion nie — ein abgeschnittener Link ist kein
 * Ziel. Das volle Meta steht dann im `title`, wenn es ein Text ist.
 */
export const PANEEL_KOPF_HOEHE = 38;

export function paneelStil(rollen: Pick<Farbrollen, 'linie' | 'paneel' | 'text'>): CSSProperties {
  return {
    display: 'flex',
    flexDirection: 'column',
    minWidth: 0,
    background: rollen.paneel,
    border: `1px solid ${rollen.linie}`,
    borderRadius: 0,
    color: rollen.text,
  };
}

export function paneelKopfStil(
  rollen: Pick<Farbrollen, 'linie'>,
  token: { padding: number; paddingXS: number },
): CSSProperties {
  return {
    display: 'flex',
    flexWrap: 'wrap',
    alignItems: 'center',
    justifyContent: 'space-between',
    columnGap: token.padding,
    rowGap: token.paddingXS,
    minHeight: PANEEL_KOPF_HOEHE,
    paddingBlock: token.paddingXS,
    paddingInline: token.padding,
    borderBlockEnd: `1px solid ${rollen.linie}`,
    flex: '0 0 auto',
  };
}

/** Rechter Block des Kopfs (Meta + Aktion): darf schrumpfen und bleibt rechtsbündig, auch
 *  wenn er in die zweite Zeile umbricht. */
export function paneelKopfRechtsStil(token: { paddingSM: number }): CSSProperties {
  return {
    display: 'flex',
    alignItems: 'center',
    gap: token.paddingSM,
    minWidth: 0,
    maxWidth: '100%',
    marginInlineStart: 'auto',
  };
}

/** Das Mono-Meta im Kopf: kürzt mit Auslassung statt überzulaufen. */
export function paneelMetaStil(rollen: Pick<Farbrollen, 'schwach'>): CSSProperties {
  return {
    ...monoStil(11),
    color: rollen.schwach,
    minWidth: 0,
    overflow: 'hidden',
    textOverflow: 'ellipsis',
    whiteSpace: 'nowrap',
  };
}

/** Eine Zeile im Paneel: Trenner `flaeche3`, Polster aus der Staffel. */
export function paneelZeileStil(
  rollen: Pick<Farbrollen, 'flaeche3'>,
  token: { padding: number; paddingSM: number },
): CSSProperties {
  return {
    paddingBlock: token.paddingSM,
    paddingInline: token.padding,
    borderBlockEnd: `1px solid ${rollen.flaeche3}`,
  };
}

/** Die zulässigen Überschriftenebenen eines Paneels — `h1` gehört dem Seitentitel. */
export type PaneelUeberschrift = 'h2' | 'h3' | 'h4' | 'h5' | 'h6';

interface PaneelProps {
  /** Wortlaut der Augenbraue — Pflicht, siehe Dateikopf. */
  titel: ReactNode;
  /** Überschriftenebene; Vorgabe `h2`. */
  ueberschrift?: PaneelUeberschrift;
  /** Mono-Meta rechts im Kopf („4 Abschnitte · 31 Einheiten"). */
  meta?: ReactNode;
  /** Aktion rechts im Kopf (Link/Knopf) — steht nach dem Meta. */
  aktion?: ReactNode;
  /** Fußzeile unter dem Körper, mit Haarlinie abgesetzt. */
  fuss?: ReactNode;
  /** Polsterung des Körpers für Fließinhalt; Vorgabe keine (Zeilen polstern selbst). */
  koerperPolster?: boolean;
  children?: ReactNode;
  style?: CSSProperties;
  className?: string;
}

export default function Paneel({
  titel,
  ueberschrift = 'h2',
  meta,
  aktion,
  fuss,
  koerperPolster = false,
  children,
  style,
  className,
}: PaneelProps) {
  const { token, rollen } = useRollen();
  const kopfId = useId();
  return (
    <section
      aria-labelledby={kopfId}
      data-lfh="paneel"
      className={className}
      style={{ ...paneelStil(rollen), ...style }}
    >
      <div style={paneelKopfStil(rollen, token)}>
        <Augenbraue
          als={ueberschrift}
          id={kopfId}
          style={{ minWidth: 0, overflowWrap: 'anywhere' }}
        >
          {titel}
        </Augenbraue>
        {(meta != null || aktion != null) && (
          <div data-lfh="paneel-kopf-rechts" style={paneelKopfRechtsStil(token)}>
            {meta != null && (
              <span
                data-lfh="paneel-meta"
                title={
                  typeof meta === 'string' || typeof meta === 'number' ? String(meta) : undefined
                }
                style={paneelMetaStil(rollen)}
              >
                {meta}
              </span>
            )}
            {aktion != null && <span style={{ flex: '0 0 auto' }}>{aktion}</span>}
          </div>
        )}
      </div>
      <div
        style={{
          flex: '1 1 auto',
          minHeight: 0,
          ...(koerperPolster ? { padding: token.padding } : {}),
        }}
      >
        {children}
      </div>
      {fuss != null && (
        <div
          style={{
            padding: `${token.paddingSM}px ${token.padding}px`,
            borderBlockStart: `1px solid ${rollen.linie}`,
          }}
        >
          {fuss}
        </div>
      )}
    </section>
  );
}

/** Zeile für Paneel-Körper — nur Optik, keine Interaktion. */
export function PaneelZeile({ children, style }: { children: ReactNode; style?: CSSProperties }) {
  const { token, rollen } = useRollen();
  return <div style={{ ...paneelZeileStil(rollen, token), ...style }}>{children}</div>;
}
