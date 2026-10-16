// Übernommen aus Lifeline Hub 3eb9115f2 (16.10.2026): frontend/src/components/instrument/Kennzahl.test.tsx
import { screen } from '@testing-library/react';
import { ConfigProvider } from 'antd';
import type { ReactElement } from 'react';
import { describe, expect, it } from 'vitest';
import { renderMitProviders } from '../../test/utils';
import { antdToken, dichten, farbenDunkel, farbenHell } from '../../theme/tokens';
import {
  Kennzahl,
  Kennzahlenband,
  kennzahlStil,
  kennzahlZielEinzug,
  kennzahlZielStil,
  kennzahlenbandStil,
  punktFarbe,
  zahlFarbe,
} from './Kennzahl';

const tokenFuer = (stufe: keyof typeof dichten) => ({
  controlHeight: dichten[stufe].zeilenhoehe,
  padding: dichten[stufe].abstand.md,
  paddingLG: dichten[stufe].abstand.lg,
  marginXS: dichten[stufe].abstand.xs,
});

describe('Kennzahl — Datenzustände (Semantik des Lage-Dashboards)', () => {
  it('Daten: Zahl, Einheit und Notiz', () => {
    renderMitProviders(
      <Kennzahl titel="Betroffene" wert={248} einheit="Pers." notiz="seit 08:00" />,
    );
    expect(screen.getByText('248')).toBeInTheDocument();
    expect(screen.getByText('Pers.')).toBeInTheDocument();
    expect(screen.getByText('seit 08:00')).toBeInTheDocument();
  });

  it('Laden: „····" mit aria-busy und „wird abgerufen" — keine Behauptung über die Menge', () => {
    renderMitProviders(<Kennzahl titel="Betroffene" wert={248} notiz="x" zustand="laden" />);
    expect(screen.getByText('····')).toHaveAttribute('aria-busy', 'true');
    expect(screen.getByText('wird abgerufen')).toBeInTheDocument();
    expect(screen.queryByText('248')).toBeNull();
  });

  it('Fehler: „?" mit title und „Stand unbekannt" — Fehler sieht nicht aus wie leer', () => {
    renderMitProviders(<Kennzahl titel="Warnstufe" wert={0} einheit="E" zustand="fehler" />);
    expect(screen.getByText('?')).toHaveAttribute('title', 'Stand unbekannt');
    expect(screen.getByText('Stand unbekannt')).toBeInTheDocument();
    expect(screen.queryByText('0')).toBeNull();
    expect(screen.queryByText('E')).toBeNull();
  });

  it('eine Null ist ein Wert, kein Zustand', () => {
    renderMitProviders(<Kennzahl titel="Vermisst" wert={0} />);
    expect(screen.getByText('0')).toBeInTheDocument();
    expect(screen.queryByText('Stand unbekannt')).toBeNull();
  });
});

describe('Kennzahl — Ton und zweiter Kanal', () => {
  it('Alarm und Achtung haben verschieden breite Kanten, neutral keine', () => {
    const alarm = kennzahlStil(farbenHell, tokenFuer('kompakt'), 'alarm', 'daten');
    const achtung = kennzahlStil(farbenHell, tokenFuer('kompakt'), 'achtung', 'daten');
    const neutral = kennzahlStil(farbenHell, tokenFuer('kompakt'), 'neutral', 'daten');
    expect(alarm.boxShadow).toBe(`inset 6px 0 0 0 ${farbenHell.alarm}`);
    expect(achtung.boxShadow).toBe(`inset 3px 0 0 0 ${farbenHell.achtung}`);
    expect(neutral.boxShadow).toBeUndefined();
  });

  it('außerhalb von `daten` gibt es weder Tonfarbe noch Kante — ein rotes „?" behauptete eine Lage', () => {
    expect(zahlFarbe(farbenDunkel, 'alarm', 'fehler')).toBe(farbenDunkel.text);
    expect(zahlFarbe(farbenDunkel, 'alarm', 'laden')).toBe(farbenDunkel.text);
    expect(zahlFarbe(farbenDunkel, 'alarm', 'daten')).toBe(farbenDunkel.alarm);
    expect(
      kennzahlStil(farbenHell, tokenFuer('kompakt'), 'alarm', 'fehler').boxShadow,
    ).toBeUndefined();
  });

  it('nachts steht die Zahl in der Tonfarbe — alle fünf Töne (Entwurf S6)', () => {
    expect(zahlFarbe(farbenDunkel, 'normal', 'daten')).toBe(farbenDunkel.normalText);
    expect(zahlFarbe(farbenDunkel, 'bedien', 'daten')).toBe(farbenDunkel.bedienText);
    expect(zahlFarbe(farbenDunkel, 'achtung', 'daten')).toBe(farbenDunkel.achtung);
    expect(zahlFarbe(farbenDunkel, 'alarm', 'daten')).toBe(farbenDunkel.alarm);
    expect(zahlFarbe(farbenDunkel, 'neutral', 'daten')).toBe(farbenDunkel.text);
  });

  it('tags ist auch die achtung-/alarm-Zahl getönt — über die Textrollen (LFH-618)', () => {
    expect(zahlFarbe(farbenHell, 'achtung', 'daten')).toBe(farbenHell.achtungText);
    expect(zahlFarbe(farbenHell, 'alarm', 'daten')).toBe(farbenHell.alarmText);
    // Die Füllfarbe als Zahl fiele unter den Tagesboden — deshalb die eigene Rolle.
    expect(zahlFarbe(farbenHell, 'achtung', 'daten')).not.toBe(farbenHell.achtung);
    expect(zahlFarbe(farbenHell, 'alarm', 'daten')).not.toBe(farbenHell.text);
    expect(zahlFarbe(farbenHell, 'normal', 'daten')).toBe(farbenHell.normalText);
    expect(zahlFarbe(farbenHell, 'bedien', 'daten')).toBe(farbenHell.bedienText);
  });

  it('normal und bedien setzen keine Kante — die Kante trennt nur die Eskalationsstufen', () => {
    for (const ton of ['normal', 'bedien'] as const) {
      expect(
        kennzahlStil(farbenHell, tokenFuer('kompakt'), ton, 'daten').boxShadow,
      ).toBeUndefined();
    }
  });

  it('trägt den wirksamen Ton als Marke — außerhalb von `daten` neutral', () => {
    const { rerender } = renderMitProviders(<Kennzahl titel="S4" wert={3} ton="bedien" />);
    expect(screen.getByText('3').closest('[data-lfh="kennzahl"]')).toHaveAttribute(
      'data-ton',
      'bedien',
    );
    rerender(<Kennzahl titel="S4" wert={3} ton="bedien" zustand="laden" />);
    expect(document.querySelector('[data-lfh="kennzahl"]')).toHaveAttribute('data-ton', 'neutral');
  });

  it('die Schrift der Zahl ändert sich mit dem Ton NICHT (kein Zeilensprung)', () => {
    renderMitProviders(
      <>
        <Kennzahl titel="A" wert="1" ton="bedien" />
        <Kennzahl titel="B" wert="2" />
      </>,
    );
    const [a, b] = screen
      .getAllByText(/^[12]$/)
      .map((e) => e.closest('[data-lfh="kennzahl-wert"]') as HTMLElement);
    expect(a.style.fontSize).toBe(b.style.fontSize);
    expect(a.style.fontWeight).toBe(b.style.fontWeight);
    expect(a.style.color).not.toBe(b.style.color);
  });
});

describe('Kennzahl — klickbar', () => {
  it('mit Ziel trägt die Zelle einen Link', () => {
    renderMitProviders(<Kennzahl titel="Betroffene" wert={248} ziel="/einsaetze/1/personen" />);
    const link = screen.getByRole('link');
    expect(link).toHaveAttribute('href', '/einsaetze/1/personen');
    expect(link).toHaveTextContent('248');
  });

  it('ohne Ziel kein Link', () => {
    renderMitProviders(<Kennzahl titel="Betroffene" wert={248} />);
    expect(screen.queryByRole('link')).toBeNull();
  });

  it('trägt den Boden aus controlHeight — 30 / 48 / 72 px — plus Polsterung', () => {
    const hoehe = (s: keyof typeof dichten) =>
      kennzahlStil(farbenHell, tokenFuer(s), 'neutral', 'daten').minHeight;
    expect(hoehe('kompakt')).toBe(30);
    expect(hoehe('komfortabel')).toBe(48);
    expect(hoehe('handschuh')).toBe(72);
    const polster = (['kompakt', 'handschuh'] as const).map(
      (s) => kennzahlStil(farbenHell, tokenFuer(s), 'neutral', 'daten').paddingBlock,
    );
    expect(polster[0]).not.toBe(polster[1]);
  });
});

describe('Kennzahl — Abstand zwischen Zielen (LFH-630, Bedien-Leitlinie Kriterium 2)', () => {
  // Literale statt Rücklesen aus dem Token (frontend/AGENTS.md): die Böden der Leitlinie sind
  // komfortabel ≥ 8 px und handschuh ≥ 16 px zwischen zwei Treffflächen, die Fuge misst 1 px.
  const FUGE = 1;

  it('der Einzug je Stufe: kompakt 0, komfortabel 4, handschuh 8', () => {
    expect(kennzahlZielEinzug(tokenFuer('kompakt'))).toBe(0);
    expect(kennzahlZielEinzug(tokenFuer('komfortabel'))).toBe(4);
    expect(kennzahlZielEinzug(tokenFuer('handschuh'))).toBe(8);
  });

  it('zwei Einzüge plus Fuge halten den Abstand der Leitlinie', () => {
    expect(2 * kennzahlZielEinzug(tokenFuer('komfortabel')) + FUGE).toBeGreaterThanOrEqual(8);
    expect(2 * kennzahlZielEinzug(tokenFuer('handschuh')) + FUGE).toBeGreaterThanOrEqual(16);
  });

  it('Einzug und Restpolsterung ergeben die Polsterung der Zelle ohne Ziel — nichts verschiebt sich', () => {
    for (const stufe of ['kompakt', 'komfortabel', 'handschuh'] as const) {
      const ohne = kennzahlStil(farbenHell, tokenFuer(stufe), 'neutral', 'daten');
      const { zelle, ziel } = kennzahlZielStil(farbenHell, tokenFuer(stufe), 'neutral', 'daten');
      const e = kennzahlZielEinzug(tokenFuer(stufe));
      expect(zelle.padding, stufe).toBe(e);
      expect(Number(ziel.paddingBlock) + e, stufe).toBe(ohne.paddingBlock);
      expect(Number(ziel.paddingInline) + e, stufe).toBe(ohne.paddingInline);
    }
  });

  it('die Trefffläche behält den Boden 30 / 48 / 72 px', () => {
    const hoehe = (s: keyof typeof dichten) =>
      kennzahlZielStil(farbenHell, tokenFuer(s), 'neutral', 'daten').ziel.minHeight;
    expect(hoehe('kompakt')).toBe(30);
    expect(hoehe('komfortabel')).toBe(48);
    expect(hoehe('handschuh')).toBe(72);
  });

  it('die Eskalationskante ist eine Auflage am Zellrand — weder Zelle noch Trefffläche tragen sie', () => {
    for (const stufe of ['kompakt', 'komfortabel', 'handschuh'] as const) {
      const { zelle, ziel, kante } = kennzahlZielStil(
        farbenHell,
        tokenFuer(stufe),
        'alarm',
        'daten',
      );
      // Am Zellgrund läge sie UNTER der Hover-Tönung des Links (kompakt: ganz verdeckt).
      expect(zelle.boxShadow, stufe).toBeUndefined();
      expect(ziel.boxShadow, stufe).toBeUndefined();
      expect(zelle.position, stufe).toBe('relative');
      expect(kante, stufe).toMatchObject({
        position: 'absolute',
        inset: 0,
        pointerEvents: 'none',
        boxShadow: `inset 6px 0 0 0 ${farbenHell.alarm}`,
      });
    }
    const achtung = kennzahlZielStil(farbenHell, tokenFuer('kompakt'), 'achtung', 'daten');
    expect(achtung.kante?.boxShadow).toBe(`inset 3px 0 0 0 ${farbenHell.achtung}`);
    expect(kennzahlZielStil(farbenHell, tokenFuer('kompakt'), 'alarm', 'laden').kante).toBeNull();
    expect(kennzahlZielStil(farbenHell, tokenFuer('kompakt'), 'bedien', 'daten').kante).toBeNull();
  });

  const imHandschuh = (ui: ReactElement) =>
    renderMitProviders(
      <ConfigProvider theme={{ token: antdToken(farbenHell, 'handschuh') }}>{ui}</ConfigProvider>,
    );

  it('mit Ziel steht der Link eingerückt in seiner Rasterzelle und trägt die Prüfanker', () => {
    imHandschuh(
      <Kennzahl
        titel="Betroffene"
        wert={248}
        ton="achtung"
        ziel="/einsaetze/1/personen"
        zielBeschriftung="Betroffene öffnen"
      />,
    );
    const link = screen.getByRole('link', { name: 'Betroffene öffnen' });
    expect(link).toHaveAttribute('href', '/einsaetze/1/personen');
    expect(link).toHaveAttribute('data-lfh', 'kennzahl');
    expect(link).toHaveAttribute('data-ton', 'achtung');
    expect(link).toHaveClass('lfh-kennzahl__ziel');
    const zelle = link.parentElement!;
    expect(zelle).toHaveAttribute('data-lfh', 'kennzahl-zelle');
    expect(zelle).toHaveClass('lfh-kennzahl');
    expect(zelle.style.padding).toBe('8px');
    expect(zelle.style.boxShadow).toBe('');
    expect(link.style.boxShadow).toBe('');
    // Die Kante liegt NACH dem Link im DOM und damit über seiner Hover-Tönung.
    const kante = zelle.querySelector<HTMLElement>('[data-lfh="kennzahl-kante"]')!;
    expect(kante).toHaveAttribute('aria-hidden', 'true');
    expect(kante.previousElementSibling).toBe(link);
    expect(kante.style.boxShadow).toMatch(/^inset 3px 0(px)? 0(px)? /);
  });

  it('ohne Eskalationston keine Kantenauflage', () => {
    imHandschuh(<Kennzahl titel="A" wert={1} ton="bedien" ziel="/x" />);
    expect(document.querySelector('[data-lfh="kennzahl-kante"]')).toBeNull();
  });

  it('auch im Zustand `laden` bleibt die Zelle mit Ziel ein eingerückter Link', () => {
    imHandschuh(<Kennzahl titel="Betroffene" wert="" zustand="laden" ziel="/x" />);
    expect(screen.getByRole('link').parentElement).toHaveAttribute('data-lfh', 'kennzahl-zelle');
  });

  it('ohne Ziel bleibt die Zelle ein einzelnes Element ohne Zellhülle', () => {
    const { container } = imHandschuh(<Kennzahl titel="Betroffene" wert={248} />);
    expect(container.querySelector('[data-lfh="kennzahl-zelle"]')).toBeNull();
    expect(container.querySelector('[data-lfh="kennzahl"]')).toHaveClass('lfh-kennzahl');
  });

  it('`style` des Aufrufers überschreibt den Stil der Trefffläche, nicht den Einzug', () => {
    imHandschuh(<Kennzahl titel="A" wert={1} ziel="/x" style={{ paddingBlock: 3 }} />);
    const link = screen.getByRole('link');
    expect(link.style.paddingBlock).toBe('3px');
    expect(link.parentElement!.style.padding).toBe('8px');
  });
});

describe('Kennzahlenband', () => {
  // LFH-961: der Grund ist `flaeche`, nicht `linie` — sonst schiene er in einer leeren Restspur
  // als graue Kachel durch. Die Fuge zeichnet die Zelle (`fugenraster.test.ts`).
  it('Fugenraster: gap 1 px, Grund flaeche, Rahmen linie', () => {
    const stil = kennzahlenbandStil(farbenHell, 5);
    expect(stil).toMatchObject({
      gap: 1,
      background: farbenHell.flaeche,
      border: `1px solid ${farbenHell.linie}`,
      gridTemplateColumns: 'repeat(5, minmax(0, 1fr))',
    });
    expect(stil.background).not.toBe(farbenHell.linie);
    expect(String(kennzahlenbandStil(farbenHell).gridTemplateColumns)).toMatch(/auto-fit/);
  });

  it('meldet sich als Fugenraster, damit jede Zelle ihre Fuge zeichnet', () => {
    renderMitProviders(
      <Kennzahlenband beschriftung="Lage in Zahlen">
        <Kennzahl titel="A" wert={1} />
      </Kennzahlenband>,
    );
    expect(screen.getByRole('group', { name: 'Lage in Zahlen' })).toHaveAttribute(
      'data-fugenraster',
    );
  });

  it('ist mit Beschriftung eine benannte Gruppe', () => {
    renderMitProviders(
      <Kennzahlenband beschriftung="Lage in Zahlen">
        <Kennzahl titel="A" wert={1} />
      </Kennzahlenband>,
    );
    expect(screen.getByRole('group', { name: 'Lage in Zahlen' })).toBeInTheDocument();
  });

  // LFH-629: die Höhe selbst setzt `sprache.css` unter `md` (Vitest fährt ohne CSS); gemessen wird
  // sie in `e2e/lagebild-cls-schmal.spec.ts`. Hier, dass nur das Band mit der Prop sie anfordert.
  it('fordert den Notizplatz unter `md` nur mit `notizZeilenSchmal` an', () => {
    renderMitProviders(
      <>
        <Kennzahlenband beschriftung="Mit" notizZeilenSchmal={2}>
          <Kennzahl titel="A" wert={1} notiz="n" />
        </Kennzahlenband>
        <Kennzahlenband beschriftung="Ohne">
          <Kennzahl titel="B" wert={2} notiz="n" />
        </Kennzahlenband>
      </>,
    );
    const mit = screen.getByRole('group', { name: 'Mit' });
    expect(mit).toHaveClass('lfh-kennzahlenband--notizzeilen');
    expect(mit.style.getPropertyValue('--lfh-kennzahl-notizzeilen')).toBe('2');
    const ohne = screen.getByRole('group', { name: 'Ohne' });
    expect(ohne).not.toHaveClass('lfh-kennzahlenband--notizzeilen');
    expect(ohne.style.getPropertyValue('--lfh-kennzahl-notizzeilen')).toBe('');
  });

  // LFH-691: Boden UND Deckel ab `md` setzt ebenfalls `sprache.css`; gemessen in
  // `e2e/kennzahlenband-notizhoehe.spec.ts`. Hier Klasse und Zeilenzahl.
  it('fordert die feste Notizhöhe ab `md` nur mit `notizZeilen` an', () => {
    renderMitProviders(
      <>
        <Kennzahlenband beschriftung="Fest" notizZeilen={3} notizZeilenSchmal={2}>
          <Kennzahl titel="A" wert={1} notiz="n" />
        </Kennzahlenband>
        <Kennzahlenband beschriftung="Frei">
          <Kennzahl titel="B" wert={2} notiz="n" />
        </Kennzahlenband>
      </>,
    );
    const fest = screen.getByRole('group', { name: 'Fest' });
    expect(fest).toHaveClass('lfh-kennzahlenband--notizfest');
    expect(fest.style.getPropertyValue('--lfh-kennzahl-notizzeilen-fest')).toBe('3');
    // Der Boden unter `md` bleibt daneben unverändert (LFH-629).
    expect(fest).toHaveClass('lfh-kennzahlenband--notizzeilen');
    expect(fest.style.getPropertyValue('--lfh-kennzahl-notizzeilen')).toBe('2');
    const frei = screen.getByRole('group', { name: 'Frei' });
    expect(frei).not.toHaveClass('lfh-kennzahlenband--notizfest');
    expect(frei.style.getPropertyValue('--lfh-kennzahl-notizzeilen-fest')).toBe('');
  });

  it('trägt den vollen Notiztext im `title`, nur im Band mit fester Notizhöhe', () => {
    const lang = 'WESER · steigend +9 cm/h · Stand 11:57 · Prognose 7,10 m bis 16:57 · +1 weitere';
    renderMitProviders(
      <>
        <Kennzahlenband beschriftung="Fest" notizZeilen={3}>
          <Kennzahl titel="Pegel" wert="6,84" notiz={lang} ziel="/x" />
        </Kennzahlenband>
        <Kennzahlenband beschriftung="Frei">
          <Kennzahl titel="Pegel" wert="6,84" notiz={lang} ziel="/y" />
        </Kennzahlenband>
      </>,
    );
    const notizIn = (band: string) =>
      screen
        .getByRole('group', { name: band })
        .querySelector<HTMLElement>('[data-lfh="kennzahl-notiz"]')!;
    expect(notizIn('Fest')).toHaveAttribute('title', lang);
    expect(notizIn('Frei')).not.toHaveAttribute('title');
    // Die Kürzung ist nur Darstellung: der zugängliche Name des Links enthält den ganzen Text.
    const link = screen
      .getByRole('group', { name: 'Fest' })
      .querySelector<HTMLElement>('a[data-lfh="kennzahl"]')!;
    expect(link).toHaveAccessibleName(expect.stringContaining('+1 weitere'));
  });

  it('setzt im festen Band außerhalb von `daten` keinen `title` an die Notiz', () => {
    const { rerender } = renderMitProviders(
      <Kennzahlenband beschriftung="Fest" notizZeilen={3}>
        <Kennzahl titel="A" wert="" notiz="x" zustand="fehler" />
      </Kennzahlenband>,
    );
    // Den Titel „Stand unbekannt“ trägt allein die Zahl.
    expect(screen.getAllByTitle('Stand unbekannt')).toHaveLength(1);
    rerender(
      <Kennzahlenband beschriftung="Fest" notizZeilen={3}>
        <Kennzahl titel="A" wert="" notiz="x" zustand="laden" />
      </Kennzahlenband>,
    );
    expect(screen.getByText('wird abgerufen')).not.toHaveAttribute('title');
  });

  it('zeigt eine Aufgliederung nur im Zustand `daten`', () => {
    const seg = { segmente: [{ label: 'SK I', wert: 2, farbe: 'red' }], titel: 'Sichtung' };
    const { rerender } = renderMitProviders(<Kennzahl titel="A" wert={2} aufgliederung={seg} />);
    expect(screen.getByRole('img', { name: 'Sichtung: SK I 2' })).toBeInTheDocument();
    rerender(<Kennzahl titel="A" wert={2} aufgliederung={seg} zustand="laden" />);
    expect(screen.queryByRole('img')).toBeNull();
  });
});

describe('Kennzahl — Statuspunkt vor der Augenbraue (S6)', () => {
  it('setzt den Punkt nur auf Wunsch, als Dekoration vor dem Stufenwort', () => {
    const { container, rerender } = renderMitProviders(<Kennzahl titel="S4" wert={17} />);
    expect(container.querySelector('[data-lfh="kennzahl-punkt"]')).toBeNull();
    rerender(<Kennzahl titel="S4" wert={17} punkt="bedien" />);
    const punkt = container.querySelector<HTMLElement>('[data-lfh="kennzahl-punkt"]')!;
    expect(punkt).toHaveAttribute('aria-hidden', 'true');
    expect(punkt).toHaveAttribute('data-ton', 'bedien');
    // VOR der Augenbraue, im selben Zeilenkopf.
    expect(punkt.nextElementSibling).toHaveTextContent('S4');
  });

  it('färbt nur im Zustand daten — laden/fehler stehen neutral am selben Platz', () => {
    const { container, rerender } = renderMitProviders(
      <Kennzahl titel="S6" wert={2} punkt="alarm" zustand="fehler" />,
    );
    expect(container.querySelector('[data-lfh="kennzahl-punkt"]')).toHaveAttribute(
      'data-ton',
      'neutral',
    );
    rerender(<Kennzahl titel="S6" wert={2} punkt="alarm" />);
    expect(container.querySelector('[data-lfh="kennzahl-punkt"]')).toHaveAttribute(
      'data-ton',
      'alarm',
    );
  });

  it('nimmt die Kantenfarbe der Statusfläche — dieselbe Rolle wie der Rand der StatusZelle', () => {
    expect(punktFarbe(farbenDunkel, 'alarm', 'daten')).toBe(farbenDunkel.alarm);
    expect(punktFarbe(farbenHell, 'normal', 'daten')).toBe(farbenHell.normal);
    expect(punktFarbe(farbenHell, 'alarm', 'laden')).toBe(farbenHell.schwach);
  });
});
