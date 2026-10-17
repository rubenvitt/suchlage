import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import './theme/rollen.css';
import './theme/sprache.css';
import './app/frame.css';
import { meldeSchriftenAn } from './theme/schriften';
import { ThemeModeProvider } from './theme/ThemeModeProvider';
import App from './app/App';

meldeSchriftenAn();

async function boot() {
  // Vorschau-Modus (`mise run ui`). Der dynamische Import hält MSW und Beispieldaten aus dem Betriebs-Build.
  if (import.meta.env.MODE === 'demo') {
    const { startDemo } = await import('./demo/startDemo');
    await startDemo();
  }
  createRoot(document.getElementById('root')!).render(
    <StrictMode>
      <ThemeModeProvider>
        <App />
      </ThemeModeProvider>
    </StrictMode>,
  );
}
void boot();
