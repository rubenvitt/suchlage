import { http, HttpResponse } from 'msw';
import type { Me } from '../contract/generated/Me';
import * as fx from '../contract/fixtures.generated';

/** Standard-Antworten aus den erzeugten Beispieldaten. `me` wählt die Rolle (Tests: Bedienung, Vorschau: Verwaltung). */
export const handlersFor = (me: Me) => [
  http.get('/api/me', () => HttpResponse.json(me)),
  http.get('/api/auth/options', () => HttpResponse.json(fx.authOptionsOidc)),
  http.post('/api/auth/logout', () => new HttpResponse(null, { status: 204 })),
  http.get('/api/situation', () => HttpResponse.json(fx.snapshotFlying)),
  http.get('/api/flights', () => HttpResponse.json(fx.flightList)),
  http.get('/api/flights/:id', () => HttpResponse.json(fx.flightDetail)),
  http.get('/api/settings', () => HttpResponse.json(fx.settingsView)),
  http.get('/api/remote-controllers', () => HttpResponse.json(fx.remoteControllers)),
  http.get('/api/kiosk-tokens', () => HttpResponse.json(fx.kioskTokens)),
];
