// Übernommen aus Lifeline Hub 3eb9115f2 (16.10.2026): frontend/src/components/instrument/Aufgliederung.test.tsx
import { screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { renderMitProviders } from '../../test/utils';
import { farbenHell } from '../../theme/tokens';
import {
  anteil,
  Aufgliederung,
  aufgliederungText,
  Balken,
  segmentFlaecheStil,
  sichtbareSegmente,
} from './Aufgliederung';

const SEGMENTE = [
  { label: 'SK I', wert: 12, farbe: 'rgb(1, 1, 1)' },
  { label: 'SK II', wert: 0, farbe: 'rgb(2, 2, 2)' },
  { label: 'SK III', wert: 96, farbe: 'rgb(3, 3, 3)' },
];

describe('Aufgliederung', () => {
  it('ist EIN Bild mit ausgeschriebenem Wortlaut — auch für Null-Segmente', () => {
    renderMitProviders(<Aufgliederung segmente={SEGMENTE} titel="Betroffene nach Sichtung" />);
    expect(
      screen.getByRole('img', { name: 'Betroffene nach Sichtung: SK I 12, SK II 0, SK III 96' }),
    ).toBeInTheDocument();
  });

  it('zeichnet nur positive Segmente, Breite nach Wert', () => {
    renderMitProviders(<Aufgliederung segmente={SEGMENTE} />);
    const bild = screen.getByRole('img');
    const teile = [...bild.children] as HTMLElement[];
    expect(teile).toHaveLength(2);
    expect(teile[0].style.flexGrow).toBe('12');
    expect(teile[1].style.flexGrow).toBe('96');
    expect(sichtbareSegmente(SEGMENTE).map((s) => s.label)).toEqual(['SK I', 'SK III']);
  });

  it('die Legende ist fürs Auge da und für Vorleser ausgeblendet', () => {
    renderMitProviders(<Aufgliederung segmente={SEGMENTE} />);
    const legende = screen.getByText('SK I 12 · SK II 0 · SK III 96');
    expect(legende).toHaveAttribute('aria-hidden', 'true');
  });

  it('eine leere Menge behält ihre Spur — kein verschwundener Balken', () => {
    renderMitProviders(<Aufgliederung segmente={[]} legende={false} />);
    expect(screen.getByRole('img')).toHaveStyle({ background: farbenHell.flaeche3 });
  });

  it('umrandet ein Segment nur auf Wunsch — ohne Layoutwirkung', () => {
    // „tot" ist schwarz und verschwände auf dem Nachtgrund; die Umrandung trägt es sichtbar.
    const tot = { label: 'tot', wert: 2, farbe: 'rgb(0, 0, 0)', umrandung: 'rgb(9, 9, 9)' };
    expect(segmentFlaecheStil(tot)).toMatchObject({
      outline: '1px solid rgb(9, 9, 9)',
      outlineOffset: -1,
      flexGrow: 2,
    });
    // Gegenprobe: ohne Angabe KEINE Umrandung — die übrigen Aufrufer bleiben unverändert.
    expect(segmentFlaecheStil(SEGMENTE[0])).not.toHaveProperty('outline');

    renderMitProviders(<Aufgliederung segmente={[SEGMENTE[0], tot]} legende={false} />);
    const teile = [...screen.getByRole('img').children] as HTMLElement[];
    expect(teile[0].style.outline).toBe('');
    expect(teile[1].style.outline).toBe('1px solid rgb(9, 9, 9)');
  });

  it('der Text ohne Titel ist die reine Aufzählung', () => {
    expect(aufgliederungText(SEGMENTE.slice(0, 1))).toBe('SK I 12');
  });
});

describe('Balken', () => {
  it('klemmt den Anteil auf 0…1 und übersteht max = 0', () => {
    expect(anteil(5, 10)).toBe(0.5);
    expect(anteil(15, 10)).toBe(1);
    expect(anteil(-1, 10)).toBe(0);
    expect(anteil(3, 0)).toBe(0);
    expect(anteil(Number.NaN, 10)).toBe(0);
  });

  it('Spur flaeche3, Füllung in der Farbe des Aufrufers, Wortlaut im Namen', () => {
    renderMitProviders(
      <Balken
        wert={6}
        max={10}
        farbe="rgb(9, 9, 9)"
        beschriftung="Nord: 60 Prozent"
        label="60 %"
      />,
    );
    const spur = screen.getByRole('img', { name: 'Nord: 60 Prozent' });
    expect(spur).toHaveStyle({ background: farbenHell.flaeche3 });
    const fuellung = spur.firstElementChild as HTMLElement;
    expect(fuellung.style.width).toBe('60%');
    expect(fuellung.style.background).toBe('rgb(9, 9, 9)');
    expect(screen.getByText('60 %')).toHaveAttribute('aria-hidden', 'true');
  });
});
