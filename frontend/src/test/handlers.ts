import { http, HttpResponse } from 'msw';
import type { KioskTokenCreate } from '../contract/generated/KioskTokenCreate';
import type { KioskTokenCreated } from '../contract/generated/KioskTokenCreated';
import type { Me } from '../contract/generated/Me';
import type { RemoteControllerCreate } from '../contract/generated/RemoteControllerCreate';
import type { RemoteControllerCreated } from '../contract/generated/RemoteControllerCreated';
import type { SetupResult } from '../contract/generated/SetupResult';
import * as fx from '../contract/fixtures.generated';

const noContent = () => new HttpResponse(null, { status: 204 });

/**
 * Standard-Antworten aus den erzeugten Beispieldaten. `me` wählt die Rolle (Tests: Bedienung, Vorschau:
 * Verwaltung). Die schreibenden Antworten tragen den Vorschau-Modus; Tests, die eine Anfrage prüfen,
 * setzen ihre eigene per `server.use`.
 */
export const handlersFor = (me: Me) => [
  http.get('/api/me', () => HttpResponse.json(me)),
  http.get('/api/auth/options', () => HttpResponse.json(fx.authOptionsOidc)),
  http.post('/api/auth/logout', noContent),
  http.get('/api/situation', () => HttpResponse.json(fx.snapshotFlying)),
  http.get('/api/flights', () => HttpResponse.json(fx.flightList)),
  http.get('/api/flights/:id', () => HttpResponse.json(fx.flightDetail)),
  http.get('/api/settings', () => HttpResponse.json(fx.settingsView)),
  http.put('/api/settings/:section', () => HttpResponse.json(fx.settingsView)),
  http.get('/api/remote-controllers', () => HttpResponse.json(fx.remoteControllers)),
  http.post('/api/remote-controllers', async ({ request }) => {
    const { label } = (await request.json()) as RemoteControllerCreate;
    const created: RemoteControllerCreated = {
      controller: { id: 3, label, sn: null, paired_at_ms: null },
      pairing_url: 'https://suchlage.example.org/pilot#vorschau',
    };
    return HttpResponse.json(created);
  }),
  http.delete('/api/remote-controllers/:id', noContent),
  http.get('/api/kiosk-tokens', () => HttpResponse.json(fx.kioskTokens)),
  http.post('/api/kiosk-tokens', async ({ request }) => {
    const { label } = (await request.json()) as KioskTokenCreate;
    const created: KioskTokenCreated = {
      token: { id: 2, label, created_at_ms: Date.now() },
      url: 'https://suchlage.example.org/kiosk#vorschau',
    };
    return HttpResponse.json(created);
  }),
  http.delete('/api/kiosk-tokens/:id', noContent),
  http.post('/api/setup', () => HttpResponse.json({ login_url: '/anmelden' } satisfies SetupResult)),
];
