// Übernommen aus Lifeline Hub 3eb9115f2 (16.10.2026): frontend/src/components/instrument/Status.test.tsx
import { screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { renderMitProviders } from '../../test/utils';
import { farbenHell } from '../../theme/tokens';
import { StatusChip, StatusZelle } from './Status';

describe('StatusZelle', () => {
  it('zeigt die Zahl auf getönter Fläche und trägt das Wort für Vorleser mit', () => {
    renderMitProviders(<StatusZelle ton="normal" wert={2} wort="bereit" />);
    const zelle = document.querySelector<HTMLElement>('[data-lfh="status-zelle"]')!;
    expect(zelle).toHaveTextContent('2 bereit');
    expect(zelle).toHaveAttribute('title', 'bereit');
    expect(zelle).toHaveStyle({
      background: farbenHell.normalFlaeche,
      color: farbenHell.normalText,
    });
  });

  it('das Wort ist nur visuell verborgen, nicht aus dem Baum genommen', () => {
    renderMitProviders(<StatusZelle ton="alarm" wert={3} wort="Ausfall" />);
    const wort = screen.getByText('Ausfall', { exact: false });
    expect(wort).not.toHaveAttribute('aria-hidden');
    expect(wort).toHaveStyle({ position: 'absolute', width: '1px' });
  });
});

describe('StatusChip', () => {
  it('Höhe 22, Fläche, Code Mono und Wort — beides sichtbar', () => {
    renderMitProviders(<StatusChip ton="bedien" code="4" wort="gebunden" />);
    const chip = document.querySelector<HTMLElement>('[data-lfh="status-chip"]')!;
    expect(chip).toHaveStyle({ height: '22px', background: farbenHell.bedienFlaeche });
    expect(screen.getByText('4')).toHaveStyle({ fontWeight: '500' });
    expect(screen.getByText('gebunden')).toBeVisible();
  });

  it('mit `trenner` steht der FMS-Punkt zwischen Code und Wort, ohne bleibt die Zahl am Wort', () => {
    renderMitProviders(
      <>
        <StatusChip ton="bedien" code="S4" wort="Am Einsatzort" trenner />
        <StatusChip ton="achtung" code={2} wort="ausstehend" />
      </>,
    );
    const [fms, zahl] = document.querySelectorAll<HTMLElement>('[data-lfh="status-chip"]');
    expect(fms.textContent).toBe('S4·Am Einsatzort');
    expect(zahl.textContent).toBe('2ausstehend');
  });

  it('ohne Code steht das Wort allein', () => {
    renderMitProviders(<StatusChip ton="neutral" wort="abgemeldet" />);
    const chip = document.querySelector<HTMLElement>('[data-lfh="status-chip"]')!;
    expect(chip.textContent).toBe('abgemeldet');
    expect(chip.dataset.ton).toBe('neutral');
  });
});
