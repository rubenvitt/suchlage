// Übernommen aus Lifeline Hub 3eb9115f2 (16.10.2026): frontend/src/components/useViewport.ts
import { useSyncExternalStore } from 'react';
import { theme } from 'antd';
import type { Breakpoint } from 'antd';

/**
 * Viewport-Primitiv (LFH-329 · B1/H24) — **der einzige erlaubte Zugang** zu Breiten- und
 * Zeigerfragen im Produktivcode, erzwungen von `useViewport.guard.test.ts`.
 *
 * ── Die Schwellen kommen aus antd und werden NICHT gespiegelt ───────────────────────────
 * `md` = 768, `lg` = 992 (antd `theme/util/alias.js`). Eine zweite Wahrheit in
 * `theme/tokens.ts` driftete still. `screenMD` am `ConfigProvider` zu überschreiben ist
 * verboten: antds `validateBreakpoints` WIRFT bei widersprüchlichen Nachbarn.
 *
 * ── `xs` ist ausgeschlossen ─────────────────────────────────────────────────────────────
 * `xs` ist als einzige Stufe eine `max-width`-Abfrage; `abBreite('xs')` läse sich als Umkehrung
 * dessen, was der Name verspricht. {@link AbBreitePunkt} schließt die Stufe am Typ aus. Dieselbe
 * Warnung gilt für die roh mitgelieferte {@link ViewportZustand.screens}-Karte.
 *
 ── Ein Satz Hörer für die ganze Seite (LFH-947) ───────────────────────────────────────
 * Die Abfragen leben in EINEM modulweiten Store (`useSyncExternalStore`): sieben Breiten aus
 * antds Schwellen (`theme.getDesignToken()`, dieselbe Karte wie `Grid.useBreakpoint`) und die
 * Zeigerfrage, also höchstens 8 Hörer, egal wie viele Zeilen fragen. Vorher hing jede
 * Zeitachsenzeile 8 eigene an (`Grid.useBreakpoint` baut seinen Beobachter je Instanz). Der
 * Store hängt sich beim ersten Abonnenten an und beim letzten ab; der Snapshot ist ein
 * stabiles Objekt, das nur bei einer geänderten Antwort neu entsteht.
 *
 * ── „Unbekannt" ist BREIT, nicht schmal ────────────────────────────────────────────────
 * Der Store antwortet schon beim ersten Render. Fehlt eine Stufe in der Karte (rohe Karte von
 * außen, `abBreiteAus`), gilt sie als breit: der Primärkontext Fükw bekommt den unkorrigierten
 * Render, statt dass für einen Frame das Handy-Layout aufblitzt.
 *
 * ── Zeigersignal: `(pointer: coarse)`, nicht `(any-pointer: coarse)` ────────────────────
 * `pointer` beschreibt den PRIMÄREN Zeiger; `any-pointer` schlüge auch am Fükw-Laptop mit
 * Touchscreen an. Das Signal belegt über {@link zeigerIstGrob} die Dichtestufe vor, wenn keine
 * Wahl gespeichert ist (LFH-361). Preis: ein 2-in-1-Tablet mit Tastatur meldet `fine` und
 * startet kompakt — lieber eine Stufe zu eng als ein Umschalter, der sich beim Neuladen
 * zurückdreht.
 *
 * ── Neben, nicht statt der Container-Abfragen ───────────────────────────────────────────
 * Die Staffelung in `theme/sprache.css` (1100/700 px) misst die Fläche des INHALTS, dieser Hook
 * den Viewport. Zwei Fragen, zwei Zahlen.
 */

/** Zeigerabfrage für Berührungsbedienung. Bewusst der primäre Zeiger — siehe Dateikopf. */
const ZEIGER_GROB = '(pointer: coarse)';

/** Breakpoints, ab denen sinnvoll „mindestens so breit" gefragt werden kann — ohne `xs`. */
export type AbBreitePunkt = Exclude<Breakpoint, 'xs'>;

/** Die Screens-Karte in antds Form (`Grid.useBreakpoint`). */
type ScreensKarte = Partial<Record<Breakpoint, boolean>>;

interface ViewportZustand {
  /**
   * Karte in antds Form. Achtung: `screens.xs` ist eine
   * `max-width`-Aussage und damit nicht wie die übrigen Stufen zu lesen.
   */
  screens: ScreensKarte;
  /** Ist der Viewport mindestens so breit wie dieser Breakpoint? Unbekannt ⇒ ja. */
  abBreite: (punkt: AbBreitePunkt) => boolean;
  /** Schmaler als `md` (768) — der Handy-/Einhand-Fall. */
  istSchmal: boolean;
  /** Primärer Zeiger ist grob (Finger/Handschuh) statt Maus/Stift. */
  istBeruehrung: boolean;
}

/**
 * Reine Ableitung, damit die Erst-Render-Semantik prüfbar ist, ohne zu rendern.
 * `!== false` statt `=== true`: `undefined` (noch unbekannt) zählt als breit.
 */
export function abBreiteAus(screens: ScreensKarte, punkt: AbBreitePunkt): boolean {
  return screens[punkt] !== false;
}

/**
 * Einmalige Momentaufnahme der Zeigerart, ohne Hook und ohne Zuhörer — für Aufrufer, die VOR
 * dem ersten Render fragen: die Vorbelegung der Dichtestufe in `theme/ThemeModeProvider.tsx`
 * (LFH-361 · B5a).
 *
 * Sie steht HIER, weil der Viewport-Guard Medienabfragen im Primitiv verlangt und
 * `ThemeModeProvider.tsx` nur für die Dunkelmodus-Frage freigestellt ist.
 *
 * Wer auf ÄNDERUNGEN reagieren muss, nimmt {@link useViewport}, nicht dies.
 */
export function zeigerIstGrob(): boolean {
  return window.matchMedia(ZEIGER_GROB).matches;
}

/** Die Breitenabfragen in antds Form (`responsiveObserver`), aus den Schwellen des Themes. */
function breitenAbfragen(): Record<Breakpoint, string> {
  const t = theme.getDesignToken();
  return {
    xs: `(max-width: ${t.screenXSMax}px)`,
    sm: `(min-width: ${t.screenSM}px)`,
    md: `(min-width: ${t.screenMD}px)`,
    lg: `(min-width: ${t.screenLG}px)`,
    xl: `(min-width: ${t.screenXL}px)`,
    xxl: `(min-width: ${t.screenXXL}px)`,
    xxxl: `(min-width: ${t.screenXXXL}px)`,
  };
}

interface Momentaufnahme {
  screens: ScreensKarte;
  istBeruehrung: boolean;
}

/**
 * Die Abfragen, gebaut beim ersten Fragen und gehalten, solange jemand abonniert. Eine
 * `MediaQueryList` antwortet über `matches` immer aktuell; Hörer hängen nur an Abonnenten.
 */
let abfragen: { breiten: [Breakpoint, MediaQueryList][]; zeiger: MediaQueryList } | null = null;
const abonnenten = new Set<() => void>();
let letzte: Momentaufnahme | null = null;

function gebaut() {
  abfragen ??= {
    breiten: Object.entries(breitenAbfragen()).map(([punkt, abfrage]) => [
      punkt as Breakpoint,
      window.matchMedia(abfrage),
    ]),
    zeiger: window.matchMedia(ZEIGER_GROB),
  };
  return abfragen;
}

function benachrichtige() {
  for (const melde of [...abonnenten]) melde();
}

function abonniere(melde: () => void): () => void {
  const { breiten, zeiger } = gebaut();
  if (abonnenten.size === 0) {
    for (const [, mql] of breiten) mql.addEventListener('change', benachrichtige);
    zeiger.addEventListener('change', benachrichtige);
  }
  abonnenten.add(melde);
  return () => {
    abonnenten.delete(melde);
    if (abonnenten.size === 0) {
      for (const [, mql] of breiten) mql.removeEventListener('change', benachrichtige);
      zeiger.removeEventListener('change', benachrichtige);
      // Ohne Abonnenten frisch bauen: die nächste Seite fragt die dann gültige Umgebung.
      abfragen = null;
    }
  };
}

/**
 * Liest die Antworten (`matches`) und gibt die letzte Momentaufnahme zurück, solange sie gleich
 * sind — `useSyncExternalStore` verlangt einen stabilen Snapshot.
 */
function momentaufnahme(): Momentaufnahme {
  const { breiten, zeiger } = gebaut();
  const istBeruehrung = zeiger.matches;
  const vorher = letzte;
  if (
    vorher &&
    vorher.istBeruehrung === istBeruehrung &&
    breiten.every(([punkt, mql]) => vorher.screens[punkt] === mql.matches)
  ) {
    return vorher;
  }
  const screens: ScreensKarte = {};
  for (const [punkt, mql] of breiten) screens[punkt] = mql.matches;
  letzte = { screens, istBeruehrung };
  return letzte;
}

/** Gemeinsame Ableitung je Momentaufnahme, damit auch `abBreite` eine stabile Referenz bleibt. */
const zustaende = new WeakMap<Momentaufnahme, ViewportZustand>();

export function useViewport(): ViewportZustand {
  const aufnahme = useSyncExternalStore(abonniere, momentaufnahme);
  let zustand = zustaende.get(aufnahme);
  if (!zustand) {
    const { screens, istBeruehrung } = aufnahme;
    const abBreite = (punkt: AbBreitePunkt) => abBreiteAus(screens, punkt);
    zustand = { screens, abBreite, istSchmal: !abBreite('md'), istBeruehrung };
    zustaende.set(aufnahme, zustand);
  }
  return zustand;
}
