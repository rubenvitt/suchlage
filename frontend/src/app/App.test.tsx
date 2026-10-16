import { render, screen } from '@testing-library/react';
import { ThemeModeProvider } from '../theme/ThemeModeProvider';
import App from './App';

test('startet im Nachtbetrieb mit Lifeline-Tokens', () => {
  render(
    <ThemeModeProvider>
      <App />
    </ThemeModeProvider>,
  );
  expect(screen.getByText('Suchlage')).toBeInTheDocument();
  expect(document.documentElement.dataset.theme).toBe('dark');
});
