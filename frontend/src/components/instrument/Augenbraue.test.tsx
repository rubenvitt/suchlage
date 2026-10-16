// Übernommen aus Lifeline Hub 3eb9115f2 (16.10.2026): frontend/src/components/instrument/Augenbraue.test.tsx
import { screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { renderMitProviders } from '../../test/utils';
import { farbenHell } from '../../theme/tokens';
import Augenbraue, { augenbraueStil } from './Augenbraue';

describe('Augenbraue', () => {
  it('ist ohne Angabe ein span — Satz, keine Gliederung', () => {
    renderMitProviders(<Augenbraue>Betroffene</Augenbraue>);
    const el = screen.getByText('Betroffene');
    expect(el.tagName).toBe('SPAN');
    expect(screen.queryByRole('heading')).toBeNull();
  });

  it('wird mit `als` zur Überschrift, ohne die Optik zu ändern', () => {
    renderMitProviders(<Augenbraue als="h2">Einsatzabschnitte</Augenbraue>);
    const h = screen.getByRole('heading', { level: 2, name: 'Einsatzabschnitte' });
    expect(h).toHaveStyle({ fontSize: '10px', fontWeight: '600', textTransform: 'uppercase' });
  });

  it('lässt den Wortlaut im DOM, wie er übergeben wird — Versalien sind CSS', () => {
    renderMitProviders(<Augenbraue>Offene Anordnungen</Augenbraue>);
    expect(screen.getByText('Offene Anordnungen').textContent).toBe('Offene Anordnungen');
  });

  it('nimmt die Farbe `schwach` des aktiven Modus', () => {
    // Blanker ConfigProvider im Test = Tagmodus.
    renderMitProviders(<Augenbraue>X</Augenbraue>);
    expect(screen.getByText('X')).toHaveStyle({ color: farbenHell.schwach });
    expect(augenbraueStil({ schwach: 'rot' }).color).toBe('rot');
  });
});
