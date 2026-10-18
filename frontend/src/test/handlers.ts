import { http, HttpResponse } from 'msw';
import type { KioskTokenCreate } from '../contract/generated/KioskTokenCreate';
import type { KioskTokenCreated } from '../contract/generated/KioskTokenCreated';
import type { MapElement } from '../contract/generated/MapElement';
import type { MapElementInput } from '../contract/generated/MapElementInput';
import type { MapElementUpdate } from '../contract/generated/MapElementUpdate';
import type { Me } from '../contract/generated/Me';
import type { Mission } from '../contract/generated/Mission';
import type { MissionContents } from '../contract/generated/MissionContents';
import type { MissionCreate } from '../contract/generated/MissionCreate';
import type { MissionUpdate } from '../contract/generated/MissionUpdate';
import type { SectionCreate } from '../contract/generated/SectionCreate';
import type { RemoteControllerCreate } from '../contract/generated/RemoteControllerCreate';
import type { RemoteControllerCreated } from '../contract/generated/RemoteControllerCreated';
import type { SetupResult } from '../contract/generated/SetupResult';
import { demoElements, demoFlightMissions, demoPhotos, matchesMission } from '../demo/missionData';
import type { MissionAssign } from '../contract/generated/MissionAssign';
import * as fx from '../contract/fixtures.generated';

/** Ein laufender und ein vorbereiteter Beispiel-Einsatz. */
export const missionFixtures = (): Mission[] => [
  {
    id: 1,
    name: 'Übung Wietze',
    location: 'Wietze',
    state: 'running',
    created_by: 'Vorschau',
    created_at_ms: 1792150000000,
    started_at_ms: 1792150600000,
    closed_at_ms: null,
    sections: [{ id: 1, label: 'Nord', location: null, created_at_ms: 1792150700000 }],
  },
  {
    id: 2,
    name: 'Hochwasser Aller',
    location: null,
    state: 'prepared',
    created_by: 'Vorschau',
    created_at_ms: 1792151000000,
    started_at_ms: null,
    closed_at_ms: null,
    sections: [],
  },
];

/** Zustandsbehaftete Einsatz-Routen mit den Regeln des Servers (ein laufender Einsatz, 409 beim Starten, 422 beim Löschen eines laufenden). */
export const missionHandlers = (initial: Mission[] = missionFixtures()) => {
  const missions = initial.map((m) => ({ ...m }));
  const find = (id: unknown) => missions.find((m) => m.id === Number(id));
  return [
    http.get('/api/missions', () => HttpResponse.json(missions)),
    http.post('/api/missions', async ({ request }) => {
      const input = (await request.json()) as MissionCreate;
      const created: Mission = {
        id: Math.max(0, ...missions.map((m) => m.id)) + 1,
        name: input.name,
        location: input.location,
        state: 'prepared',
        created_by: 'Vorschau',
        created_at_ms: Date.now(),
        started_at_ms: null,
        closed_at_ms: null,
        sections: [],
      };
      missions.push(created);
      return HttpResponse.json(created);
    }),
    http.post('/api/missions/:id/start', ({ params }) => {
      const m = find(params.id);
      if (!m) return HttpResponse.json({ error: 'Nicht gefunden' }, { status: 404 });
      if (missions.some((o) => o.state === 'running'))
        return HttpResponse.json({ error: 'Es läuft schon ein Einsatz' }, { status: 409 });
      m.state = 'running';
      m.started_at_ms = Date.now();
      return HttpResponse.json(m);
    }),
    http.post('/api/missions/:id/close', ({ params }) => {
      const m = find(params.id);
      if (!m) return HttpResponse.json({ error: 'Nicht gefunden' }, { status: 404 });
      m.state = 'closed';
      m.closed_at_ms = Date.now();
      return HttpResponse.json(m);
    }),
    http.put('/api/missions/:id', async ({ params, request }) => {
      const m = find(params.id);
      if (!m) return HttpResponse.json({ error: 'Nicht gefunden' }, { status: 404 });
      Object.assign(m, (await request.json()) as MissionUpdate);
      return HttpResponse.json(m);
    }),
    // Feste Mengen: Die Vorschau zählt keine echten Flüge und Fotos.
    http.get('/api/missions/:id/contents', () =>
      HttpResponse.json({ flights: 3, photos: 41, map_elements: 4, video_segments: 2 } satisfies MissionContents),
    ),
    http.delete('/api/missions/:id', ({ params }) => {
      const m = find(params.id);
      if (!m) return HttpResponse.json({ error: 'Nicht gefunden' }, { status: 404 });
      if (m.state === 'running')
        return HttpResponse.json({ error: 'Ein laufender Einsatz lässt sich nicht löschen' }, { status: 422 });
      missions.splice(missions.indexOf(m), 1);
      return new HttpResponse(null, { status: 204 });
    }),
    http.post('/api/missions/:id/sections', async ({ params, request }) => {
      const m = find(params.id);
      if (!m) return HttpResponse.json({ error: 'Nicht gefunden' }, { status: 404 });
      const input = (await request.json()) as SectionCreate;
      m.sections.push({
        id: m.sections.length + 1,
        label: input.label,
        location: input.location,
        created_at_ms: Date.now(),
      });
      return HttpResponse.json(m);
    }),
  ];
};

const noContent = () => new HttpResponse(null, { status: 204 });

/**
 * Standard-Antworten aus den erzeugten Beispieldaten. `me` wählt die Rolle (Tests: Bedienung, Vorschau:
 * Verwaltung). Die schreibenden Antworten tragen den Vorschau-Modus; Tests, die eine Anfrage prüfen,
 * setzen ihre eigene per `server.use`.
 */
export const handlersFor = (me: Me) => {
  // Im Vorschau-Modus gezeichnete Elemente, damit Umbenennen ihre Form behält.
  const drawn = new Map<string, MapElement>(demoElements.map((e) => [e.id, e]));
  const flightMissions = demoFlightMissions();
  return [
    http.get('/api/me', () => HttpResponse.json(me)),
    http.get('/api/health', () => HttpResponse.json(fx.health)),
    http.get('/api/auth/options', () => HttpResponse.json(fx.authOptionsOidc)),
    http.post('/api/auth/logout', noContent),
    http.get('/api/situation', () => HttpResponse.json(fx.snapshotFlying)),
    http.get('/api/flights', ({ request }) =>
      HttpResponse.json(
        fx.flightList.filter((f) => matchesMission(request.url, flightMissions.get(f.id) ?? null)),
      ),
    ),
    http.put('/api/flights/:id/mission', async ({ request, params }) => {
      flightMissions.set(Number(params.id), ((await request.json()) as MissionAssign).mission_id);
      return noContent();
    }),
    http.get('/api/flights/:id', () => HttpResponse.json(fx.flightDetail)),
    http.get('/api/flights/:id/photos', () => HttpResponse.json(fx.photoShots)),
    http.get('/api/photos', ({ request }) =>
      HttpResponse.json(demoPhotos.filter((s) => matchesMission(request.url, s.mission_id))),
    ),
    // Vorschau-Modus: ein Platzhalter statt eines echten Vorschaubilds.
    http.get(
      '/api/photos/:id/thumb',
      () =>
        new HttpResponse(
          '<svg xmlns="http://www.w3.org/2000/svg" width="640" height="512"><rect width="640" height="512" fill="#3a2a52"/><circle cx="320" cy="256" r="60" fill="#f2a33a"/></svg>',
          { headers: { 'content-type': 'image/svg+xml' } },
        ),
    ),
    ...missionHandlers(),
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
    http.get('/api/map/elements', ({ request }) =>
      HttpResponse.json([...drawn.values()].filter((e) => matchesMission(request.url, e.mission_id))),
    ),
    http.put('/api/map/elements/:id/mission', async ({ request, params }) => {
      const known = drawn.get(String(params.id));
      if (!known) return HttpResponse.json({ error: 'Nicht gefunden' }, { status: 404 });
      const changed = {
        ...known,
        mission_id: ((await request.json()) as MissionAssign).mission_id,
        updated_at_ms: Date.now(),
      };
      drawn.set(changed.id, changed);
      return HttpResponse.json(changed);
    }),
    http.post('/api/map/elements', async ({ request }) => {
      const input = (await request.json()) as MapElementInput;
      const now = Date.now();
      const created: MapElement = {
        id: crypto.randomUUID(),
        name: input.name,
        kind: input.kind,
        points: input.points.map((p) => ({ ...p, alt_m: null })),
        color: input.color,
        clamp_to_ground: true,
        author: me.display_name,
        origin: 'web',
        created_at_ms: now,
        updated_at_ms: now,
        mission_id: input.mission_id,
      };
      drawn.set(created.id, created);
      return HttpResponse.json(created);
    }),
    http.put('/api/map/elements/:id', async ({ request, params }) => {
      const known = drawn.get(String(params.id));
      if (!known) return HttpResponse.json({ error: 'Nicht gefunden' }, { status: 404 });
      const changed: MapElement = {
        ...known,
        ...((await request.json()) as MapElementUpdate),
        updated_at_ms: Date.now(),
      };
      drawn.set(changed.id, changed);
      return HttpResponse.json(changed);
    }),
    http.delete('/api/map/elements/:id', ({ params }) => {
      drawn.delete(String(params.id));
      return noContent();
    }),
  ];
};
