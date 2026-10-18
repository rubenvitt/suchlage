// Rendert `public/apple-touch-icon.png` (180 × 180) aus `public/favicon.svg` mit Playwright-Chromium.
// Aufruf nach jeder Änderung der Bildmarke: `node scripts/render-icons.ts` (im Ordner frontend).
// Das Favicon ist die Quelle; `src/brand/brandMark.guard.test.tsx` hält es auf der Geometrie.
import { readFileSync } from 'node:fs';
import { chromium } from '@playwright/test';

const SIZE = 180;
const svg = readFileSync(new URL('../public/favicon.svg', import.meta.url), 'utf8');
const target = new URL('../public/apple-touch-icon.png', import.meta.url).pathname;

const browser = await chromium.launch();
try {
  const page = await browser.newPage({ viewport: { width: SIZE, height: SIZE } });
  await page.setContent(
    `<body style="margin:0"><img width="${SIZE}" height="${SIZE}" style="display:block" src="data:image/svg+xml;base64,${Buffer.from(svg).toString('base64')}"></body>`,
  );
  await page.screenshot({ path: target, omitBackground: false });
} finally {
  await browser.close();
}
console.log(`geschrieben: ${target}`);
