import { defineConfig, devices } from '@playwright/test';

export default defineConfig({
  testDir: 'e2e',
  timeout: 90_000,
  // scripts/e2e.sh setzt E2E_BASE_URL auf den Server mit freiem Port.
  use: { baseURL: process.env.E2E_BASE_URL ?? 'http://127.0.0.1:8080', screenshot: 'on', trace: 'retain-on-failure' },
  projects: [
    { name: 'arbeitsplatz', use: { ...devices['Desktop Chrome'], viewport: { width: 1440, height: 900 } } },
    { name: 'grossbild', use: { ...devices['Desktop Chrome'], viewport: { width: 1920, height: 1080 } } },
    { name: 'mobil', use: { ...devices['Pixel 7'] } },
  ],
});
