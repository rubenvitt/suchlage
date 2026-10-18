// Übernommen aus Lifeline Hub 5f07a20de (18.10.2026): frontend/src/icons/IconRahmen.test.tsx
import { render } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { icon, iconHuelleStil } from './IconRahmen';

const Probe = icon('probe', '0 0 50 50', [
  { d: 'M 0 0 L 50 50 Z' },
  { d: 'M 1 1', fillRule: 'evenodd' },
]);

function huelle(container: HTMLElement): HTMLElement {
  const el = container.querySelector<HTMLElement>('[data-lfh-icon="probe"]');
  if (!el) throw new Error('Hülle fehlt');
  return el;
}

describe('IconRahmen — ein Icon des Satzes (LFH-595)', () => {
  it('wächst ohne Größenangabe mit der Schrift: 1em', () => {
    const { container } = render(<Probe />);
    const svg = huelle(container).querySelector('svg');
    expect(svg?.getAttribute('width')).toBe('1em');
    expect(svg?.getAttribute('height')).toBe('1em');
  });

  it('nimmt eine feste Kantenlänge', () => {
    const { container } = render(<Probe size={20} />);
    const svg = huelle(container).querySelector('svg');
    expect(svg?.getAttribute('width')).toBe('20');
    expect(svg?.getAttribute('height')).toBe('20');
  });

  it('übernimmt die Textfarbe und behält Raster und Pfade der Quelle', () => {
    const { container } = render(<Probe />);
    const svg = huelle(container).querySelector('svg');
    expect(svg?.getAttribute('fill')).toBe('currentColor');
    expect(svg?.getAttribute('viewBox')).toBe('0 0 50 50');
    const pfade = svg?.querySelectorAll('path') ?? [];
    expect(pfade).toHaveLength(2);
    expect(pfade[0].getAttribute('d')).toBe('M 0 0 L 50 50 Z');
    expect(pfade[1].getAttribute('fill-rule')).toBe('evenodd');
    expect(pfade[0].hasAttribute('fill')).toBe(false);
  });

  it('ist für assistive Technik verborgen und nicht fokussierbar', () => {
    const { container } = render(<Probe />);
    const el = huelle(container);
    expect(el.getAttribute('aria-hidden')).toBe('true');
    expect(el.querySelector('svg')?.getAttribute('focusable')).toBe('false');
    expect(el.querySelector('svg')?.getAttribute('role')).toBeNull();
  });

  it('trägt die Klassen, über die antd Icons in seinen Bauteilen ausrichtet', () => {
    const { container } = render(<Probe className="extra" />);
    const el = huelle(container);
    expect(el.classList.contains('anticon')).toBe(true);
    expect(el.classList.contains('lfh-icon')).toBe(true);
    expect(el.classList.contains('extra')).toBe(true);
    expect(el.classList.contains('lfh-icon-dreht')).toBe(false);
  });

  it('dreht nur auf Wunsch (Ladeanzeige)', () => {
    const { container } = render(<Probe drehen />);
    expect(huelle(container).classList.contains('lfh-icon-dreht')).toBe(true);
  });

  it('richtet sich aus wie ein antd-Icon und lässt eigene Stile zu', () => {
    expect(iconHuelleStil).toMatchObject({
      display: 'inline-flex',
      alignItems: 'center',
      color: 'inherit',
      lineHeight: 0,
      verticalAlign: '-0.125em',
    });
    const { container } = render(<Probe style={{ marginInlineEnd: 4 }} />);
    expect(huelle(container).style.marginInlineEnd).toBe('4px');
    expect(huelle(container).style.display).toBe('inline-flex');
  });

  it('nennt sich im React-Baum nach ihrem Registernamen', () => {
    expect(Probe.displayName).toBe('Icon(probe)');
  });
});
