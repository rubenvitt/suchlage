// Übernommen aus Lifeline Hub 3eb9115f2 (16.10.2026): frontend/src/components/instrument/statusFlaeche.test.ts
import { describe, expect, it } from 'vitest';
import { kontrast } from '../../test/farbmass';
import { farbenDunkel, farbenHell } from '../../theme/tokens';
import { statusFlaeche, tonVonRolle, type StatusTon } from './statusFlaeche';

/**
 * Die Kontrastzusicherung der Statusfläche, GERECHNET statt behauptet (WCAG-Formel).
 * Böden aus Kriterium 5 der Bedien-Leitlinie, als Literale: Tag ≥ 7, Nacht ≥ 5 für den
 * Text; ≥ 3 für die Kante gegen ihren Grund (WCAG 1.4.11, wie `kraefte-kontrast.spec.ts`).
 * Aus der Palette gelesen und nicht aus einer Konstante: ändert jemand eine Rolle in
 * `tokens.ts`, wird dieser Test rot, bevor die e2e-Suite es wird.
 */

const TOENE: StatusTon[] = ['normal', 'achtung', 'alarm', 'bedien', 'neutral'];

describe('statusFlaeche — Kontrast', () => {
  it.each(TOENE)('Tag/%s: Text ≥ 7 : 1, Kante ≥ 3 : 1', (ton) => {
    const f = statusFlaeche(farbenHell, ton);
    expect(kontrast(f.text, f.grund)).toBeGreaterThanOrEqual(7);
    expect(kontrast(f.kante, f.grund)).toBeGreaterThanOrEqual(3);
  });

  it.each(TOENE)('Nacht/%s: Text ≥ 5 : 1, Kante ≥ 3 : 1', (ton) => {
    const f = statusFlaeche(farbenDunkel, ton);
    expect(kontrast(f.text, f.grund)).toBeGreaterThanOrEqual(5);
    expect(kontrast(f.kante, f.grund)).toBeGreaterThanOrEqual(3);
  });

  it('nachts ist der Text getönt wie im Entwurf', () => {
    expect(statusFlaeche(farbenDunkel, 'achtung').text).toBe(farbenDunkel.achtung);
    expect(statusFlaeche(farbenDunkel, 'alarm').text).toBe(farbenDunkel.alarm);
    // Nachts SIND die Textrollen die Entwurfswerte.
    expect(farbenDunkel.achtungText).toBe(farbenDunkel.achtung);
    expect(farbenDunkel.alarmText).toBe(farbenDunkel.alarm);
    expect(statusFlaeche(farbenDunkel, 'normal').text).toBe(farbenDunkel.normalText);
  });

  /**
   * Das Paar aus LFH-618: die FÜLLFARBE trägt am Tag den Boden nicht — deshalb gibt es die
   * Textrollen, und die Beschriftung ist GETÖNT, nicht `text`. Bis dahin wich der Tag auf
   * `text` aus; die Kontrastspecs waren grün, aber die Ampel war am Tag farblos.
   */
  it('am Tag trägt die Füllfarbe den Boden nicht — die Beschriftung nimmt die Textrolle', () => {
    expect(kontrast(farbenHell.achtung, farbenHell.achtungFlaeche)).toBeLessThan(7);
    expect(kontrast(farbenHell.alarm, farbenHell.alarmFlaeche)).toBeLessThan(7);
    expect(statusFlaeche(farbenHell, 'achtung').text).toBe(farbenHell.achtungText);
    expect(statusFlaeche(farbenHell, 'alarm').text).toBe(farbenHell.alarmText);
    expect(statusFlaeche(farbenHell, 'achtung').text).not.toBe(farbenHell.text);
    expect(statusFlaeche(farbenHell, 'alarm').text).not.toBe(farbenHell.text);
    // Die Kante bleibt die Füllfarbe.
    expect(statusFlaeche(farbenHell, 'alarm').kante).toBe(farbenHell.alarm);
  });

  it('neutral ist flaeche3 + text2, die Kante schwach', () => {
    for (const farben of [farbenDunkel, farbenHell]) {
      expect(statusFlaeche(farben, 'neutral')).toEqual({
        grund: farben.flaeche3,
        text: farben.text2,
        kante: farben.schwach,
      });
    }
  });

  it('marke hat keine Statusfläche', () => {
    expect(tonVonRolle('marke')).toBeNull();
    expect(tonVonRolle('alarm')).toBe('alarm');
  });
});
