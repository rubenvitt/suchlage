// Übernommen aus Lifeline Hub 3eb9115f2 (16.10.2026): frontend/src/components/instrument/Segmentleiste.test.tsx
import { screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { useState } from 'react';
import { describe, expect, it, vi } from 'vitest';
import { renderMitProviders } from '../../test/utils';
import { dichten } from '../../theme/tokens';
import Segmentleiste, {
  naechsterIndex,
  segmentStil,
  segmentZelleStil,
  type SegmentOption,
} from './Segmentleiste';

const OPTIONEN: SegmentOption<string>[] = [
  { wert: 'alle', label: 'Alle' },
  { wert: 'meldung', label: 'Meldung', punkt: 'rgb(22, 119, 255)' },
  { wert: 'lage', label: 'Lage' },
];

function Gesteuert({ rolle }: { rolle?: 'radiogroup' | 'tablist' }) {
  const [wert, setWert] = useState('alle');
  return (
    <Segmentleiste
      optionen={OPTIONEN}
      wert={wert}
      onWechsel={setWert}
      beschriftung="Einträge nach Typ"
      rolle={rolle}
    />
  );
}

describe('Segmentleiste', () => {
  it('ist eine benannte Radiogruppe mit genau einem gewählten Segment', () => {
    renderMitProviders(<Gesteuert />);
    expect(screen.getByRole('radiogroup', { name: 'Einträge nach Typ' })).toBeInTheDocument();
    expect(screen.getByRole('radio', { name: 'Alle' })).toBeChecked();
    expect(screen.getAllByRole('radio', { checked: true })).toHaveLength(1);
  });

  it('wählt per Klick', async () => {
    const onWechsel = vi.fn();
    renderMitProviders(
      <Segmentleiste optionen={OPTIONEN} wert="alle" onWechsel={onWechsel} beschriftung="x" />,
    );
    await userEvent.click(screen.getByRole('radio', { name: 'Lage' }));
    expect(onWechsel).toHaveBeenCalledWith('lage');
  });

  it('Pfeiltasten wandern und wählen, Fokus folgt; nur das gewählte Segment liegt im Tab-Weg', async () => {
    renderMitProviders(<Gesteuert />);
    const alle = screen.getByRole('radio', { name: 'Alle' });
    expect(alle).toHaveAttribute('tabindex', '0');
    expect(screen.getByRole('radio', { name: 'Lage' })).toHaveAttribute('tabindex', '-1');
    alle.focus();
    await userEvent.keyboard('{ArrowRight}');
    expect(screen.getByRole('radio', { name: 'Meldung' })).toBeChecked();
    expect(screen.getByRole('radio', { name: 'Meldung' })).toHaveFocus();
    await userEvent.keyboard('{ArrowLeft}{ArrowLeft}');
    expect(screen.getByRole('radio', { name: 'Lage' })).toBeChecked();
  });

  it('als Tabliste trägt es Tabs mit aria-selected', () => {
    renderMitProviders(<Gesteuert rolle="tablist" />);
    expect(screen.getByRole('tablist')).toBeInTheDocument();
    expect(screen.getByRole('tab', { name: 'Alle' })).toHaveAttribute('aria-selected', 'true');
  });

  it('der Farbpunkt ist Dekoration — der Name bleibt der Wortlaut', () => {
    renderMitProviders(<Gesteuert />);
    const meldung = screen.getByRole('radio', { name: 'Meldung' });
    const punkt = meldung.querySelector('[data-lfh="segment-punkt"]');
    expect(punkt).toHaveAttribute('aria-hidden', 'true');
  });

  it('naechsterIndex läuft im Kreis und kennt Pos1/Ende', () => {
    expect(naechsterIndex('ArrowRight', 2, 3)).toBe(0);
    expect(naechsterIndex('ArrowLeft', 0, 3)).toBe(2);
    expect(naechsterIndex('Home', 2, 3)).toBe(0);
    expect(naechsterIndex('End', 0, 3)).toBe(2);
    expect(naechsterIndex('a', 0, 3)).toBeNull();
  });

  it('naechsterIndex überspringt gesperrte Segmente, auch an den Rändern', () => {
    const gesperrt = (i: number) => i === 1;
    expect(naechsterIndex('ArrowRight', 0, 3, gesperrt)).toBe(2);
    expect(naechsterIndex('ArrowLeft', 2, 3, gesperrt)).toBe(0);
    expect(naechsterIndex('Home', 2, 3, (i) => i === 0)).toBe(1);
    expect(naechsterIndex('End', 0, 3, (i) => i === 2)).toBe(1);
    // Alles gesperrt: nichts wandert.
    expect(naechsterIndex('ArrowRight', 0, 3, () => true)).toBeNull();
  });

  it('ein gesperrtes Segment bleibt sichtbar, nennt seinen Grund und wählt nicht', async () => {
    const onWechsel = vi.fn();
    const optionen: SegmentOption<string>[] = [
      { wert: 'online', label: 'Online' },
      { wert: 'offline', label: 'Offline', gesperrt: 'Keine Offline-Karte hinterlegt' },
      { wert: 'luft', label: 'Luftbild' },
    ];
    renderMitProviders(
      <Segmentleiste
        optionen={optionen}
        wert="online"
        onWechsel={onWechsel}
        beschriftung="Karte"
      />,
    );
    const offline = screen.getByRole('radio', { name: 'Offline' });
    // `aria-disabled`, nicht `disabled`: das Segment bleibt erreichbar, sonst wäre der Grund es
    // nicht. Der Grund ist die Beschreibung — ein zweiter Kanal neben dem Grau.
    expect(offline).toHaveAttribute('aria-disabled', 'true');
    expect(offline).not.toBeDisabled();
    expect(offline).toHaveAccessibleDescription('Keine Offline-Karte hinterlegt');
    await userEvent.click(offline);
    expect(onWechsel).not.toHaveBeenCalled();
    // Pfeil rechts springt über das gesperrte Segment hinweg.
    screen.getByRole('radio', { name: 'Online' }).focus();
    await userEvent.keyboard('{ArrowRight}');
    expect(onWechsel).toHaveBeenCalledWith('luft');
    // Gegenprobe: ein freies Segment trägt weder Sperre noch Beschreibung.
    const online = screen.getByRole('radio', { name: 'Online' });
    expect(online).not.toHaveAttribute('aria-disabled');
    expect(online).not.toHaveAttribute('aria-describedby');
  });

  /** Gate 3: ZWEI Angaben an einem handgebauten Bedienziel, Böden als Literale. */
  it('Bedienziel: Boden 30 / 48 / 72 aus controlHeight, plus Polsterung, die mitwächst', () => {
    const t = (s: keyof typeof dichten) => ({
      controlHeight: dichten[s].zeilenhoehe,
      paddingSM: dichten[s].abstand.sm,
      marginXS: dichten[s].abstand.xs,
    });
    expect(segmentStil(t('kompakt')).minHeight).toBe(30);
    expect(segmentStil(t('komfortabel')).minHeight).toBe(48);
    expect(segmentStil(t('handschuh')).minHeight).toBe(72);
    expect(segmentStil(t('kompakt')).paddingInline).not.toBe(
      segmentStil(t('handschuh')).paddingInline,
    );
  });

  /**
   * Zielabstand (LFH-865, Muster LFH-630): die Fuge bleibt 1 px, das Segment rückt in seiner
   * Zelle ein. Soll als Literale aus der Bedien-Leitlinie (Kriterium 2).
   */
  describe('Zielabstand im Fugenraster', () => {
    const FUGE = 1;
    const t = (s: keyof typeof dichten) => ({
      controlHeight: dichten[s].zeilenhoehe,
      paddingSM: dichten[s].abstand.sm,
      marginXS: dichten[s].abstand.xs,
    });

    it('der Einzug je Stufe: kompakt 0, komfortabel 4, handschuh 8', () => {
      expect(segmentZelleStil(t('kompakt')).padding).toBe(0);
      expect(segmentZelleStil(t('komfortabel')).padding).toBe(4);
      expect(segmentZelleStil(t('handschuh')).padding).toBe(8);
    });

    it('zwei Einzüge plus Fuge halten ≥ 8 (komfortabel) und ≥ 16 (handschuh)', () => {
      expect(2 * Number(segmentZelleStil(t('komfortabel')).padding) + FUGE).toBeGreaterThanOrEqual(
        8,
      );
      expect(2 * Number(segmentZelleStil(t('handschuh')).padding) + FUGE).toBeGreaterThanOrEqual(
        16,
      );
    });

    it('Einzug und Restpolsterung ergeben die Polsterung von bisher — die Breite bleibt', () => {
      for (const stufe of ['kompakt', 'komfortabel', 'handschuh'] as const) {
        const e = Number(segmentZelleStil(t(stufe)).padding);
        expect(Number(segmentStil(t(stufe)).paddingInline) + e, stufe).toBe(
          dichten[stufe].abstand.sm,
        );
      }
    });

    it('jedes Segment steht in einer eigenen Zelle ohne Rolle, die Gruppe besitzt die Segmente', () => {
      renderMitProviders(<Gesteuert rolle="tablist" />);
      const tabs = screen.getAllByRole('tab');
      expect(tabs).toHaveLength(3);
      for (const tab of tabs) {
        const zelle = tab.parentElement!;
        expect(zelle).toHaveAttribute('data-lfh', 'segment-zelle');
        expect(zelle).toHaveAttribute('role', 'none');
        expect(zelle).toHaveClass('lfh-segment-zelle');
        expect(zelle.parentElement).toHaveAttribute('role', 'tablist');
      }
    });

    it('auch ein gesperrtes Segment steht in seiner Zelle (der Tooltip liegt darin)', () => {
      renderMitProviders(
        <Segmentleiste
          optionen={[
            { wert: 'a', label: 'A' },
            { wert: 'b', label: 'B', gesperrt: 'Grund' },
          ]}
          wert="a"
          onWechsel={() => {}}
          beschriftung="x"
        />,
      );
      const b = screen.getByRole('radio', { name: 'B' });
      expect(b.parentElement).toHaveAttribute('data-lfh', 'segment-zelle');
    });
  });
});
