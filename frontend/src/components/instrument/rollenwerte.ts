// Übernommen aus Lifeline Hub 3eb9115f2 (16.10.2026): frontend/src/components/instrument/rollenwerte.ts
import { theme, type GlobalToken } from 'antd';
import {
  farbenDunkel,
  farbenHell,
  schrift,
  schriftskala,
  type Farbrollen,
  type Schriftstufenname,
} from '../../theme/tokens';

/**
 * Die Farbrollen des AKTIVEN Modus für die Instrumenten-Bausteine (Neuentwurf
 * „Instrumententafel", `docs/design/2026-09-21-neuentwurf/umsetzung.md`).
 *
 * WARUM NICHT NUR `theme.useToken()`: antd kennt `paneel`, `kopf`, `flaeche3`, `text2`,
 * die deckenden Statusflächen, das Sammelbanner und die Zeilentönungen nicht —
 * `antdToken()` bildet sie auf keinen Token ab. TSX liest sie deshalb aus den Paletten in
 * `theme/tokens.ts`, CSS aus `--lfh-*` in `theme/rollen.css` (die Arbeitsteilung „ZWEI
 * QUELLEN, EINE WAHRHEIT"). Kein Farbwert steht hier — Gate 5.
 *
 * DER MODUS: dieselbe Helligkeitsprobe auf `colorBgBase`, die `theme/statusFarben.ts`
 * (`istDunklerModus`, dort privat) für `marke`, Flächen, ETB-Typ und Warnstufenbalken
 * fährt — nicht `useThemeMode().effektiv`. Der Kontext liefert ohne Provider `dark`
 * (Vorgabe seit dem Neuentwurf), das antd-Theme eines blanken `ConfigProvider` ist aber
 * hell: zwei Quellen würden im Test zwei Modi melden. Die Probe am Token hängt dagegen an
 * genau dem Theme, das auch die antd-Flächen daneben färbt. Ein fremdes Theme fällt auf
 * Hell zurück, statt zu werfen.
 */
export function istDunkel(token: Pick<GlobalToken, 'colorBgBase'>): boolean {
  const kurz = token.colorBgBase.trim().replace('#', '');
  const hex = kurz.length === 3 ? [...kurz].map((z) => z + z).join('') : kurz;
  if (hex.length < 6) return false;
  const wert = Number.parseInt(hex.slice(0, 6), 16);
  if (Number.isNaN(wert)) return false;
  // Relative Helligkeit nach ITU-R BT.709 — dieselbe Gewichtung wie in statusFarben.ts.
  const helligkeit =
    0.2126 * ((wert >> 16) & 255) + 0.7152 * ((wert >> 8) & 255) + 0.0722 * (wert & 255);
  return helligkeit < 128;
}

/** Die Farbrollen des Modus, den `token` beschreibt. Rein — ohne Render prüfbar. */
export function rollenwerte(token: Pick<GlobalToken, 'colorBgBase'>): Farbrollen {
  return istDunkel(token) ? farbenDunkel : farbenHell;
}

/** Token und Rollen in einem Griff — der Standardzugang aller Bausteine. */
export function useRollen(): { token: GlobalToken; rollen: Farbrollen; dunkel: boolean } {
  const { token } = theme.useToken();
  const dunkel = istDunkel(token);
  return { token, rollen: dunkel ? farbenDunkel : farbenHell, dunkel };
}

/**
 * Eine Stufe der Schriftskala als Inline-Stil. Familie aus `schrift`, Werte aus
 * `schriftskala` — keine zweite Skala. Zahlenfamilien bekommen `tabular-nums` mit
 * (Signatur 3: Ziffernflattern ist ein Lesefehler).
 */
export function schriftStil(stufe: Schriftstufenname) {
  const s: {
    groesse: number;
    gewicht: number;
    familie: keyof typeof schrift;
    sperrung?: string;
    versal?: boolean;
  } = schriftskala[stufe];
  return {
    fontFamily: schrift[s.familie],
    fontSize: s.groesse,
    fontWeight: s.gewicht,
    ...(s.sperrung ? { letterSpacing: s.sperrung } : {}),
    ...(s.versal ? { textTransform: 'uppercase' as const } : {}),
    ...(s.familie === 'zahl' ? { fontVariantNumeric: 'tabular-nums' as const } : {}),
  };
}

/** Mono in beliebiger Größe (Meta 10/11, Zeiten 12/13, Präfix 14) — Familie aus `schrift.zahl`. */
export function monoStil(groesse: number, gewicht: 400 | 500 = 400) {
  return {
    fontFamily: schrift.zahl,
    fontSize: groesse,
    fontWeight: gewicht,
    fontVariantNumeric: 'tabular-nums' as const,
  };
}
