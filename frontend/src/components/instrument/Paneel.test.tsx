// Übernommen aus Lifeline Hub 3eb9115f2 (16.10.2026): frontend/src/components/instrument/Paneel.test.tsx
import { screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { renderMitProviders } from '../../test/utils';
import { farbenHell } from '../../theme/tokens';
import Paneel, {
  PANEEL_KOPF_HOEHE,
  paneelKopfRechtsStil,
  paneelKopfStil,
  paneelMetaStil,
  paneelStil,
  paneelZeileStil,
} from './Paneel';

describe('Paneel', () => {
  it('ist eine Region, benannt über ihre Augenbrauen-Überschrift', () => {
    renderMitProviders(
      <Paneel titel="Einsatzabschnitte" meta="4 Abschnitte · 31 Einheiten">
        <p>Inhalt</p>
      </Paneel>,
    );
    const region = screen.getByRole('region', { name: 'Einsatzabschnitte' });
    expect(region).toContainElement(screen.getByText('Inhalt'));
    expect(screen.getByRole('heading', { level: 2, name: 'Einsatzabschnitte' })).toBeVisible();
    expect(screen.getByText('4 Abschnitte · 31 Einheiten')).toBeInTheDocument();
  });

  it('nimmt die Überschriftenebene vom Aufrufer', () => {
    renderMitProviders(<Paneel titel="Nächste Marken" ueberschrift="h3" />);
    expect(screen.getByRole('heading', { level: 3, name: 'Nächste Marken' })).toBeVisible();
  });

  it.each([5, 6] as const)('trägt auch tiefe Gliederung (h%i) — die Optik bleibt dieselbe', (n) => {
    renderMitProviders(<Paneel titel="Tief" ueberschrift={`h${n}`} />);
    const kopf = screen.getByRole('heading', { level: n, name: 'Tief' });
    expect(kopf).toHaveClass('lfh-augenbraue');
    expect(screen.getByRole('region', { name: 'Tief' })).toBeInTheDocument();
  });

  it('rendert Aktion und Fuß nur, wenn sie übergeben werden', () => {
    const { rerender } = renderMitProviders(<Paneel titel="A" />);
    expect(screen.queryByRole('button')).toBeNull();
    rerender(
      <Paneel titel="A" aktion={<button type="button">ETB</button>} fuss={<span>Fuß</span>} />,
    );
    expect(screen.getByRole('button', { name: 'ETB' })).toBeInTheDocument();
    expect(screen.getByText('Fuß')).toBeInTheDocument();
  });

  it('Rahmen linie, Grund paneel, Radius 0 — aus den Rollen', () => {
    expect(paneelStil(farbenHell)).toMatchObject({
      background: farbenHell.paneel,
      border: `1px solid ${farbenHell.linie}`,
      borderRadius: 0,
    });
  });

  it('der Kopf hat 38 px als Boden, nicht als feste Höhe — eine Aktion darf ihn dehnen', () => {
    const stil = paneelKopfStil(farbenHell, { padding: 11, paddingXS: 3 });
    expect(PANEEL_KOPF_HOEHE).toBe(38);
    expect(stil.minHeight).toBe(38);
    expect(stil.height).toBeUndefined();
  });

  it('ein enger Kopf bricht um und kürzt das Meta — die Aktion bleibt ganz (1024-px-Befund)', () => {
    // jsdom rechnet kein Layout; belegt werden die Angaben, aus denen das Verhalten folgt.
    // Ohne `flexWrap` und `minWidth: 0` schob `space-between` den rechten Block über den
    // Rand in den Nachbarkopf (Lage-Dashboard, drei Paneele nebeneinander).
    expect(paneelKopfStil(farbenHell, { padding: 11, paddingXS: 3 }).flexWrap).toBe('wrap');
    expect(paneelKopfRechtsStil({ paddingSM: 7 })).toMatchObject({
      minWidth: 0,
      maxWidth: '100%',
      marginInlineStart: 'auto',
    });
    expect(paneelMetaStil(farbenHell)).toMatchObject({
      minWidth: 0,
      overflow: 'hidden',
      textOverflow: 'ellipsis',
      whiteSpace: 'nowrap',
    });
    renderMitProviders(
      <Paneel titel="Gefahren" meta="5 Gebiete" aktion={<button type="button">Gefahren</button>} />,
    );
    const meta = screen.getByText('5 Gebiete');
    expect(meta).toHaveAttribute('title', '5 Gebiete');
    // Die Aktion steht NICHT im kürzenden Meta, sondern daneben mit festem Maß.
    expect(meta).not.toContainElement(screen.getByRole('button', { name: 'Gefahren' }));
    expect(screen.getByRole('button', { name: 'Gefahren' }).parentElement).toHaveStyle({
      flex: '0 0 auto',
    });
  });

  it('Zeilen trennt flaeche3 (Entwurf #16191d)', () => {
    expect(paneelZeileStil(farbenHell, { padding: 11, paddingSM: 7 }).borderBlockEnd).toBe(
      `1px solid ${farbenHell.flaeche3}`,
    );
  });
});
