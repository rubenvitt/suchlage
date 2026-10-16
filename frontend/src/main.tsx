import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import './theme/rollen.css';
import './theme/sprache.css';
import './app/frame.css';
import { meldeSchriftenAn } from './theme/schriften';
import { ThemeModeProvider } from './theme/ThemeModeProvider';
import App from './app/App';

meldeSchriftenAn();

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <ThemeModeProvider>
      <App />
    </ThemeModeProvider>
  </StrictMode>,
);
