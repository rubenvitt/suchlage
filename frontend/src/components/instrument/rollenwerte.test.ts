// Übernommen aus Lifeline Hub 3eb9115f2 (16.10.2026): frontend/src/components/instrument/rollenwerte.test.ts
import { describe, expect, it } from 'vitest';
import { farbenDunkel, farbenHell, schrift } from '../../theme/tokens';
import { istDunkel, monoStil, rollenwerte, schriftStil } from './rollenwerte';

describe('rollenwerte', () => {
  it('erkennt den Modus an der Helligkeit der Basisfläche', () => {
    expect(istDunkel({ colorBgBase: '#000' })).toBe(true);
    expect(istDunkel({ colorBgBase: '#000000' })).toBe(true);
    expect(istDunkel({ colorBgBase: '#fff' })).toBe(false);
    expect(istDunkel({ colorBgBase: '#ffffff' })).toBe(false);
  });

  it('fällt bei einem fremden Wert auf Hell zurück, statt zu werfen', () => {
    expect(istDunkel({ colorBgBase: 'transparent' })).toBe(false);
    expect(istDunkel({ colorBgBase: '' })).toBe(false);
  });

  it('liefert die Palette des erkannten Modus — keine Kopie, dieselbe Referenz', () => {
    expect(rollenwerte({ colorBgBase: '#000' })).toBe(farbenDunkel);
    expect(rollenwerte({ colorBgBase: '#fff' })).toBe(farbenHell);
  });

  it('die Augenbraue kommt aus der Schriftskala: 10 px, 600, .14em, Versalien', () => {
    expect(schriftStil('augenbraue')).toEqual({
      fontFamily: schrift.text,
      fontSize: 10,
      fontWeight: 600,
      letterSpacing: '0.14em',
      textTransform: 'uppercase',
    });
  });

  it('Zahlenstufen laufen in Mono mit tabular-nums', () => {
    expect(schriftStil('datenwert')).toMatchObject({
      fontFamily: schrift.zahl,
      fontSize: 32,
      fontWeight: 500,
      fontVariantNumeric: 'tabular-nums',
    });
    expect(monoStil(11)).toEqual({
      fontFamily: schrift.zahl,
      fontSize: 11,
      fontWeight: 400,
      fontVariantNumeric: 'tabular-nums',
    });
  });
});
