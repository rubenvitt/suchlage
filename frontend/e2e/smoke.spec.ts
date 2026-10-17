import { expect, test, type Page } from '@playwright/test';

/** Ungefangene Fehler der Seite; ein Fehler im Karten-Load ließe sonst nur still die Lage verschwinden. */
function pageErrors(page: Page): string[] {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  return errors;
}

async function devLogin(page: Page) {
  await page.goto('/anmelden');
  await page.getByRole('button', { name: 'Als Entwicklung anmelden' }).click();
  await expect(page.getByText('SYNC')).toBeVisible({ timeout: 15_000 });
}

/** Karte und Livebild stehen; erst dann zeigt ein Bildschirmfoto die Ansicht, wie sie im Einsatz aussieht. */
async function expectMapAndVideo(page: Page) {
  await expect(page.locator('.maplibregl-canvas')).toBeVisible();
  await expect
    .poll(
      () =>
        page
          .locator('video')
          .first()
          .evaluate((v: HTMLVideoElement) => v.readyState),
      { timeout: 45_000 },
    )
    .toBeGreaterThanOrEqual(2);
}

test('Arbeitsplatz: Karte, Telemetrie, Livebild und laufender Flug', async ({ page }, info) => {
  test.skip(info.project.name !== 'arbeitsplatz');
  const errors = pageErrors(page);
  await devLogin(page);
  const strip = page.getByRole('region', { name: 'Telemetrie' });
  await expect(strip.locator('[data-tone]').first()).toBeVisible();
  await expect(page.getByText('Flug läuft')).toBeVisible({ timeout: 60_000 });
  await expectMapAndVideo(page);
  // Gegenprobe zu „Mobil“: Hier bedient jemand, also steht die Linsenwahl da.
  await expect(page.getByRole('radiogroup', { name: 'Linse wählen' })).toBeVisible();
  await page.screenshot({ path: info.outputPath('arbeitsplatz.png'), fullPage: true });
  expect(errors).toEqual([]);
});

test('Großbild: über Zugangslink, ohne Bedienelemente', async ({ page, browser }, info) => {
  test.skip(info.project.name !== 'grossbild');
  await devLogin(page);
  const res = await page.request.post('/api/kiosk-tokens', { data: { label: 'E2E Großbild' } });
  expect(res.ok()).toBeTruthy();
  const { url } = (await res.json()) as { url: string };
  const context = await browser.newContext({ viewport: { width: 1920, height: 1080 } });
  const kiosk = await context.newPage();
  const errors = pageErrors(kiosk);
  await kiosk.goto(url.replace('http://127.0.0.1:8080', ''));
  await expect(kiosk.getByRole('region', { name: 'Telemetrie' })).toHaveAttribute('data-size', 'large', {
    timeout: 15_000,
  });
  await expectMapAndVideo(kiosk);
  await expect(kiosk.getByRole('button')).toHaveCount(0);
  await kiosk.screenshot({ path: info.outputPath('grossbild.png') });
  expect(errors).toEqual([]);
  await context.close();
});

test('Mobil: Zuschauen ohne Steuerung', async ({ page }, info) => {
  test.skip(info.project.name !== 'mobil');
  const errors = pageErrors(page);
  await devLogin(page);
  await expect(
    page.getByRole('navigation', { name: 'Bereiche' }).getByRole('link', { name: 'Flüge' }),
  ).toBeVisible();
  await expect(page.getByRole('region', { name: 'Telemetrie' })).toHaveAttribute('data-size', 'mobile');
  await expectMapAndVideo(page);
  // Läuft das Livebild schon, hieße der Knopf „Livebild beenden“: deshalb jede Livebild- und Linsenbedienung.
  await expect(page.getByRole('button', { name: /Livebild/ })).toHaveCount(0);
  await expect(page.getByRole('radiogroup', { name: 'Linse wählen' })).toHaveCount(0);
  await page.screenshot({ path: info.outputPath('mobil.png'), fullPage: true });
  expect(errors).toEqual([]);
});
