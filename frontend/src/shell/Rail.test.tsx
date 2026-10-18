import { screen } from '@testing-library/react';
import { renderWithApp } from '../test/render';
import { Rail } from './Rail';

function iconIn(name: string): string | null {
  return (
    screen.getByRole('link', { name }).querySelector('[data-lfh-icon]')?.getAttribute('data-lfh-icon') ?? null
  );
}

test('jeder Bereich zeigt sein Icon, der Name bleibt das Etikett', () => {
  renderWithApp(<Rail role="admin" />, { route: '/flights' });
  expect(iconIn('Lage')).toBe('karte');
  expect(iconIn('Einsätze')).toBe('klemmbrett');
  expect(iconIn('Flüge')).toBe('papierflieger');
  expect(iconIn('System')).toBe('monitor');
  expect(iconIn('Einstellungen')).toBe('zahnrad');
});

test('der aktive Bereich zeigt das gefüllte Icon', () => {
  renderWithApp(<Rail role="admin" />, { route: '/settings' });
  expect(screen.getByRole('link', { name: 'Einstellungen' })).toHaveAttribute('aria-current', 'page');
  expect(iconIn('Einstellungen')).toBe('zahnrad.gefuellt');
  expect(iconIn('Lage')).toBe('karte');
});

test('Einsatzkräfte sehen nur Lage, Einsätze und Flüge', () => {
  renderWithApp(<Rail role="operator" />);
  expect(screen.getAllByRole('link').map((a) => a.textContent)).toEqual(['Lage', 'Einsätze', 'Flüge']);
  expect(iconIn('Lage')).toBe('karte.gefuellt');
});
