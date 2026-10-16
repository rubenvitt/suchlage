// Übernommen aus Lifeline Hub 3eb9115f2 (16.10.2026): frontend/src/components/instrument/Kennzahl.tsx
import { createContext, useContext, type CSSProperties, type ReactNode } from 'react';
import { Link } from 'react-router';
import type { Farbrollen, Schriftstufenname } from '../../theme/tokens';
import Augenbraue from './Augenbraue';
import { Aufgliederung, type Segment } from './Aufgliederung';
import { monoStil, schriftStil, useRollen } from './rollenwerte';
import { statusFlaeche, type StatusTon } from './statusFlaeche';
import { zielEinzug } from './zielEinzug';
import { fugenrasterGrund } from './fugenraster';
// Grund und Hover/Fokus der Zelle stehen als Klasse `.lfh-kennzahl` in der Gestaltungssprache:
// ein Inline-Grund schlüge jede `:hover`-Regel.
import '../../theme/sprache.css';

/**
 * Kennzahl und Kennzahlenband — „Zahl führt".
 *
 * Eine Zelle: Augenbraue · Zahl Mono 500 in drei Stufen (22 / 32 / 40 aus `schriftskala`) ·
 * optionale Einheit Mono 12 `schwach` · Notiz 11 `gedaempft` · optionaler Aufgliederungsbalken.
 * Das Band setzt n Zellen ins FUGENRASTER: `gap: 1px` auf `linie`, jede Zelle auf `flaeche`.
 * Die Fuge bleibt in jeder Dichtestufe 1 px; den Abstand klickbarer Zellen hält der Einzug
 * (siehe „KLICKBAR“).
 *
 * ── DATENZUSTÄNDE (wie im Lage-Dashboard, LFH-331 · B3) ────────────────────────────
 *
 * `laden` → „····" mit `aria-busy`, Notiz „wird abgerufen". `fehler` → „?" mit
 * `title="Stand unbekannt"`, Notiz „Stand unbekannt". Die Strings pinnen die Tests von
 * `pages/lage-dashboard/LageDashboardPage.tsx`. Eine NULL ist ein Wert, kein Zustand — die drei
 * Formen sehen verschieden aus. Der Ton gilt nur im Zustand `daten`: ein „?" in Alarmrot
 * behauptete eine Lage, die niemand kennt.
 *
 * ── TON UND ZWEITER KANAL ──────────────────────────────────────────────────────────
 *
 * `achtung`/`alarm` färben die Zahl UND setzen eine abgestufte Innenkante (3 bzw. 6 px,
 * `inset`-Schatten, null Layout): die beiden Eskalationsstufen unterscheiden sich auch ohne
 * Farbe, und die Zeile springt nicht.
 *
 * `normal`/`bedien` (etwa „frei", „am Einsatzort" im Meldebild) färben nur die Zahl, ohne
 * Kante: ein Zustand ohne Handlungsbedarf bekommt keine. Der zweite Kanal ist dort das Wort in
 * Augenbraue und Notiz.
 *
 * KONTRAST DER ZAHL (WCAG, gegen `flaeche` als Grund von `.lfh-kennzahl`; Boden Tag ≥ 7 : 1,
 * Nacht ≥ 5 : 1):
 *
 * | Ton     | Rolle der Zahl | Tag (auf #ffffff) | Nacht (auf #0f1215) |
 * |---------|----------------|-------------------|---------------------|
 * | normal  | normalText     | 9,18              | 10,92               |
 * | bedien  | bedienText     | 9,51              |  9,95               |
 * | achtung | achtungText    | 9,22              | 11,75               |
 * | alarm   | alarmText      | 8,96              |  6,77               |
 *
 * `neutral` steht in `text` (Tag 18,47). Die Füllfarben selbst tragen den Tagesboden als Text
 * nicht, deshalb die `…Text`-Rollen.
 *
 * ── NOTIZHÖHE (LFH-629, LFH-691) ──────────────────────────────────────────────────
 *
 * Eine Notiz, deren Länge sich mit der Lage ändert, ließe das Band wachsen und schrumpfen und
 * alles darunter springen (Prüfliste Kriterium 12). Das Band fordert deshalb Platz an, gesetzt in
 * `sprache.css` vom ersten Bild an: unter `md` mit `notizZeilenSchmal` einen Boden (LFH-629), ab
 * `md` mit `notizZeilen` Boden UND Deckel (LFH-691). Eine längere Notiz endet dort mit „…“; der
 * volle Text bleibt im DOM, also im zugänglichen Namen, und steht im Zustand `daten` im `title`.
 * Die tragende Aussage einer Notiz steht deshalb vorn. Herleitung:
 * `openspec/changes/archive/2026-10-01-lfh-691-kennzahl-notiz-feste-hoehe/design.md`.
 *
 * HANDSCHUH UNTER `md` (LFH-883): die Polsterung `paddingLG` (44 px) lässt auf 390 px eine
 * Textspalte von rund 94 px (kompakt 146, komfortabel 126). Darin bricht eine Augenbraue wie
 * „Schäden offen“ zweizeilig um und die Pegel-Notiz dreizeilig, die Platzhalter aber tragen eine
 * Zeile Augenbraue und „wird abgerufen“: die Reihen wuchsen mit den Daten um 13 bzw. 16 px. In
 * dieser Stufe hält das Band deshalb zwei Zeilen Augenbraue und eine Notizzeile mehr als
 * `notizZeilenSchmal` (`sprache.css`, an `[data-dichte]`, damit der Platz vom ersten Bild an steht).
 *
 * ── STATUSPUNKT ────────────────────────────────────────────────────────────────────
 *
 * `punkt` setzt ein 8-px-Quadrat in der Tonfarbe VOR die Augenbraue — die Kachel benennt damit
 * eine STUFE (S1 … S6), keine Bewertung der Zahl; deshalb getrennt von `ton`. Dekoration
 * (`aria-hidden`), der zweite Kanal ist die Augenbraue. Nur im Zustand `daten` getönt, sonst
 * neutral an seinem Platz.
 *
 * ── KLICKBAR ────────────────────────────────────────────────────────────────────────
 *
 * Mit `ziel` trägt die Zelle einen `<Link>` — ein handgebautes Bedienziel mit den ZWEI
 * Angaben aus LFH-365. Auch im Zustand `laden` bleibt sie ein Link: das Ziel existiert ohne
 * Zahl.
 *
 * ABSTAND ZWISCHEN ZIELEN (LFH-630, Bedien-Leitlinie Kriterium 2): im Fugenraster stünden zwei
 * Links nur 1 px auseinander, die Leitlinie verlangt in `komfortabel` ≥ 8 px und in `handschuh`
 * ≥ 16 px. Die Fuge bleibt deshalb 1 px, und der Link rückt INNERHALB seiner Rasterzelle um
 * {@link kennzahlZielEinzug} (0 / 4 / 8 px) von jedem Rand ab: zwischen zwei Treffflächen liegen
 * Zellfläche, Fuge, Zellfläche. Die Polsterung des Links sinkt um denselben Betrag, Zahl und
 * Notiz stehen also wie ohne Ziel. Die Zelle (`data-lfh="kennzahl-zelle"`) trägt den Grund, der
 * Link Inhalt, Treffhöhe und die Prüfanker (`data-lfh="kennzahl"`, `data-ton`), eine Auflage
 * nach dem Link die Eskalationskante (`data-lfh="kennzahl-kante"`, siehe
 * {@link kennzahlZielStil}). Hover und Fokus liegen als `.lfh-kennzahl__ziel` in `sprache.css` auf dem Link:
 * die Tönung zeigt, wo ein Tippen wirkt, der Rand bleibt still. Herleitung und verworfene
 * Wege (breitere Fuge, Kacheln):
 * `openspec/changes/archive/2026-10-01-lfh-630-kennzahlenband-handschuh-abstand/design.md`.
 */

export type KennzahlZustand = 'daten' | 'laden' | 'fehler';
export type KennzahlTon = 'neutral' | 'normal' | 'bedien' | 'achtung' | 'alarm';
type KennzahlGroesse = 'klein' | 'mittel' | 'gross';

/** Ob das umgebende Band eine feste Notizhöhe hat (`Kennzahlenband notizZeilen`). */
const NotizFestKontext = createContext(false);

const STUFE: Record<KennzahlGroesse, Schriftstufenname> = {
  klein: 'datenwertKlein',
  mittel: 'datenwert',
  gross: 'datenwertGross',
};

/** Kantenbreite je Ton — abgestuft, damit Alarm und Achtung ohne Farbe unterscheidbar sind. */
const KANTE: Record<KennzahlTon, number> = {
  neutral: 0,
  normal: 0,
  bedien: 0,
  achtung: 3,
  alarm: 6,
};

const LADE_ZEICHEN = '····';
const FEHLER_ZEICHEN = '?';
const STAND_UNBEKANNT = 'Stand unbekannt';
const WIRD_ABGERUFEN = 'wird abgerufen';

/** Die Zahlfarbe — nur im Zustand `daten` getönt, Kontrast siehe Dateikopf. Rein. */
export function zahlFarbe(
  rollen: Pick<Farbrollen, 'text' | 'normalText' | 'bedienText' | 'achtungText' | 'alarmText'>,
  ton: KennzahlTon,
  zustand: KennzahlZustand,
): string {
  if (zustand !== 'daten') return rollen.text;
  switch (ton) {
    case 'normal':
      return rollen.normalText;
    case 'bedien':
      return rollen.bedienText;
    case 'achtung':
      return rollen.achtungText;
    case 'alarm':
      return rollen.alarmText;
    case 'neutral':
      return rollen.text;
  }
}

/**
 * Farbe des Statuspunkts — die KANTE der Statusfläche (`statusFlaeche`), dieselbe Rollenfarbe
 * wie der Rand von `StatusZelle`/`StatusChip`. Außerhalb von `daten` neutral. Rein.
 */
export function punktFarbe(
  rollen: Parameters<typeof statusFlaeche>[0],
  ton: StatusTon,
  zustand: KennzahlZustand,
): string {
  return statusFlaeche(rollen, zustand === 'daten' ? ton : 'neutral').kante;
}

/**
 * Stil der Zelle — rein und exportiert (Muster `bedienzielStil`). `minHeight` steht IMMER da,
 * nicht nur klickbar: ein Band mit gemischten Zellen springt sonst in der Höhe.
 */
export function kennzahlStil(
  rollen: Pick<Farbrollen, 'text' | 'achtung' | 'alarm'>,
  token: { controlHeight: number; padding: number; paddingLG: number; marginXS: number },
  ton: KennzahlTon,
  zustand: KennzahlZustand,
): CSSProperties {
  const kante = zustand === 'daten' ? KANTE[ton] : 0;
  return {
    display: 'flex',
    flexDirection: 'column',
    gap: token.marginXS,
    minWidth: 0,
    minHeight: token.controlHeight,
    paddingBlock: token.padding,
    paddingInline: token.paddingLG,
    color: rollen.text,
    textDecoration: 'none',
    ...(kante > 0
      ? { boxShadow: `inset ${kante}px 0 0 0 ${ton === 'alarm' ? rollen.alarm : rollen.achtung}` }
      : {}),
  };
}

/**
 * Einzug der Trefffläche in ihrer Rasterzelle je Dichtestufe — siehe Dateikopf „Abstand
 * zwischen Zielen“. Seit LFH-865 teilen Segmentleiste und Kartenknöpfe denselben Einzug; Werte
 * und Begründung stehen in {@link zielEinzug}.
 */
export function kennzahlZielEinzug(token: { controlHeight: number }): number {
  return zielEinzug(token);
}

/**
 * Stile der klickbaren Zelle — rein und exportiert. `zelle` ist die Fläche im Fugenraster
 * (Einzug als Polsterung), `ziel` der Link mit dem Zellstil aus {@link kennzahlStil}, dessen
 * Polsterung um den Einzug sinkt, `kante` die Eskalationskante als Auflage (`null` ohne Kante).
 *
 * WARUM EINE AUFLAGE: ein `inset`-Schatten malt über dem Grund SEINES Elements, aber unter
 * dessen Kindern. An der Zelle verdeckte ihn die Hover-Tönung des Links — in `kompakt` (Einzug
 * 0, Maus am Fükw) ganz, in `komfortabel` zur Hälfte der Alarmkante. Die Auflage steht nach dem
 * Link, deckt die Zelle (`inset: 0`), nimmt keinen Treffer und kein Layout: die Kante bleibt am
 * Zellrand und über jeder Tönung.
 */
export function kennzahlZielStil(
  rollen: Pick<Farbrollen, 'text' | 'achtung' | 'alarm'>,
  token: { controlHeight: number; padding: number; paddingLG: number; marginXS: number },
  ton: KennzahlTon,
  zustand: KennzahlZustand,
): { zelle: CSSProperties; ziel: CSSProperties; kante: CSSProperties | null } {
  const einzug = kennzahlZielEinzug(token);
  const { boxShadow, ...zellstil } = kennzahlStil(rollen, token, ton, zustand);
  return {
    zelle: { position: 'relative', display: 'flex', minWidth: 0, padding: einzug },
    ziel: {
      ...zellstil,
      flex: '1 1 auto',
      paddingBlock: token.padding - einzug,
      paddingInline: token.paddingLG - einzug,
    },
    kante:
      boxShadow != null
        ? { position: 'absolute', inset: 0, pointerEvents: 'none', boxShadow }
        : null,
  };
}

interface KennzahlProps {
  /** Augenbraue — was gezählt wird. */
  titel: ReactNode;
  wert: ReactNode;
  einheit?: ReactNode;
  notiz?: ReactNode;
  groesse?: KennzahlGroesse;
  ton?: KennzahlTon;
  zustand?: KennzahlZustand;
  /** Statuspunkt vor der Augenbraue (S6-Statusstufen) — siehe Dateikopf. */
  punkt?: StatusTon;
  /** Aufgliederungsbalken unter der Zahl. */
  aufgliederung?: { segmente: readonly Segment[]; titel?: string; legende?: ReactNode | false };
  /** Macht die Zelle zum Link auf diese Route (gebaut über `routing/deeplinks.ts`). */
  ziel?: string;
  /** Zugänglicher Name des Links; Vorgabe ist sein Textinhalt. */
  zielBeschriftung?: string;
  style?: CSSProperties;
}

export function Kennzahl({
  titel,
  wert,
  einheit,
  notiz,
  groesse = 'mittel',
  ton = 'neutral',
  zustand = 'daten',
  punkt,
  aufgliederung,
  ziel,
  zielBeschriftung,
  style,
}: KennzahlProps) {
  const { token, rollen } = useRollen();
  const zahl =
    zustand === 'laden' ? (
      <b aria-busy="true" style={{ font: 'inherit' }}>
        {LADE_ZEICHEN}
      </b>
    ) : zustand === 'fehler' ? (
      <b title={STAND_UNBEKANNT} style={{ font: 'inherit' }}>
        {FEHLER_ZEICHEN}
      </b>
    ) : (
      <b style={{ font: 'inherit' }}>{wert}</b>
    );
  const notizText =
    zustand === 'fehler' ? STAND_UNBEKANNT : zustand === 'laden' ? WIRD_ABGERUFEN : notiz;
  // Im Band mit fester Notizhöhe kann die Notiz gekürzt sein: der volle Text als Hinweis (siehe
  // Dateikopf „NOTIZHÖHE“). Nur in `daten` — im Zustand `fehler` trägt schon die Zahl den Titel.
  const notizFest = useContext(NotizFestKontext);
  const notizTitel =
    notizFest && zustand === 'daten' && typeof notiz === 'string' ? notiz : undefined;

  const inhalt = (
    <>
      {punkt != null ? (
        <span style={{ display: 'flex', alignItems: 'center', gap: token.marginXS }}>
          <span
            aria-hidden="true"
            data-lfh="kennzahl-punkt"
            data-ton={zustand === 'daten' ? punkt : 'neutral'}
            style={{
              width: 8,
              height: 8,
              flex: '0 0 8px',
              background: punktFarbe(rollen, punkt, zustand),
            }}
          />
          <Augenbraue className="lfh-kennzahl__titel">{titel}</Augenbraue>
        </span>
      ) : (
        <Augenbraue className="lfh-kennzahl__titel">{titel}</Augenbraue>
      )}
      <span style={{ display: 'flex', alignItems: 'baseline', gap: token.marginXS }}>
        <span
          data-lfh="kennzahl-wert"
          style={{
            ...schriftStil(STUFE[groesse]),
            lineHeight: 1,
            color: zahlFarbe(rollen, ton, zustand),
          }}
        >
          {zahl}
        </span>
        {/* `lineHeight: 1` wie die Zahl: mit der normalen Zeilenhöhe ragte die Einheit an der
            Grundlinie über die Zahl hinaus, und die Zeile wuchs mit den Daten um 1 px (LFH-883). */}
        {einheit != null && zustand === 'daten' && (
          <span style={{ ...monoStil(12), lineHeight: 1, color: rollen.schwach }}>{einheit}</span>
        )}
      </span>
      {aufgliederung != null && zustand === 'daten' && (
        <Aufgliederung
          segmente={aufgliederung.segmente}
          titel={aufgliederung.titel}
          legende={aufgliederung.legende}
        />
      )}
      {notizText != null && (
        <span
          data-lfh="kennzahl-notiz"
          title={notizTitel}
          style={{
            fontSize: 11,
            lineHeight: 1.4,
            color: rollen.gedaempft,
            overflowWrap: 'anywhere',
          }}
        >
          {notizText}
        </span>
      )}
    </>
  );

  // Der WIRKSAME Ton als Marke (Prüfanker für e2e-Kontrast und Tests): außerhalb von `daten`
  // neutral, wie Farbe und Kante.
  const datenTon: KennzahlTon = zustand === 'daten' ? ton : 'neutral';
  if (ziel != null) {
    const { zelle, ziel: zielStil, kante } = kennzahlZielStil(rollen, token, ton, zustand);
    return (
      <div className="lfh-kennzahl" data-lfh="kennzahl-zelle" style={zelle}>
        <Link
          to={ziel}
          aria-label={zielBeschriftung}
          className="lfh-kennzahl__ziel"
          data-lfh="kennzahl"
          data-ton={datenTon}
          style={{ ...zielStil, ...style }}
        >
          {inhalt}
        </Link>
        {kante != null && <span aria-hidden="true" data-lfh="kennzahl-kante" style={kante} />}
      </div>
    );
  }
  return (
    <div
      className="lfh-kennzahl"
      data-lfh="kennzahl"
      data-ton={datenTon}
      aria-busy={zustand === 'laden' || undefined}
      style={{ ...kennzahlStil(rollen, token, ton, zustand), ...style }}
    >
      {inhalt}
    </div>
  );
}

/**
 * Stil des Fugenrasters. `spalten` fest, sonst so viele, wie mit ≥ 160 px passen —
 * das Band bricht auf dem Handschirm um, statt Etiketten abzuschneiden. Grund und Fuge aus
 * {@link fugenrasterGrund}; wer den Stil übernimmt, setzt auch `data-fugenraster`.
 */
export function kennzahlenbandStil(
  rollen: Pick<Farbrollen, 'linie' | 'flaeche'>,
  spalten?: number,
): CSSProperties {
  return {
    display: 'grid',
    gridTemplateColumns:
      spalten != null
        ? `repeat(${spalten}, minmax(0, 1fr))`
        : 'repeat(auto-fit, minmax(min(160px, 100%), 1fr))',
    ...fugenrasterGrund(rollen),
  };
}

interface KennzahlenbandProps {
  children: ReactNode;
  /** Feste Spaltenzahl; ohne sie füllt das Band nach verfügbarer Breite. */
  spalten?: number;
  /** Zugänglicher Name der Gruppe („Lage in Zahlen"). */
  beschriftung?: string;
  /**
   * Platz jeder Notiz unter `md` in Zeilen, vom ersten Bild an (LFH-629, `sprache.css`): im
   * Ladezustand ist die Notiz einzeilig, eine längere Daten-Notiz bräche in der halben
   * Bandbreite um und schöbe alles darunter. Ohne die Prop wächst die Notiz mit ihrem Text.
   */
  notizZeilenSchmal?: number;
  /**
   * Höhe jeder Notiz ab `md` in Zeilen, als Boden UND Deckel (LFH-691, `sprache.css`): eine
   * längere Notiz endet mit „…“ und trägt den vollen Text im `title`. Ohne die Prop wächst die
   * Notiz ab `md` mit ihrem Text.
   */
  notizZeilen?: number;
  style?: CSSProperties;
}

export function Kennzahlenband({
  children,
  spalten,
  beschriftung,
  notizZeilenSchmal,
  notizZeilen,
  style,
}: KennzahlenbandProps) {
  const { rollen } = useRollen();
  const klassen = [
    notizZeilenSchmal != null && 'lfh-kennzahlenband--notizzeilen',
    notizZeilen != null && 'lfh-kennzahlenband--notizfest',
  ].filter(Boolean);
  return (
    <div
      role={beschriftung ? 'group' : undefined}
      aria-label={beschriftung}
      data-lfh="kennzahlenband"
      data-fugenraster=""
      className={klassen.length > 0 ? klassen.join(' ') : undefined}
      style={{
        ...kennzahlenbandStil(rollen, spalten),
        ...(notizZeilenSchmal != null
          ? ({ '--lfh-kennzahl-notizzeilen': notizZeilenSchmal } as CSSProperties)
          : {}),
        ...(notizZeilen != null
          ? ({ '--lfh-kennzahl-notizzeilen-fest': notizZeilen } as CSSProperties)
          : {}),
        ...style,
      }}
    >
      <NotizFestKontext.Provider value={notizZeilen != null}>{children}</NotizFestKontext.Provider>
    </div>
  );
}
