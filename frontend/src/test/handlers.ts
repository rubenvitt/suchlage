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
import type { MissionFlight } from '../contract/generated/MissionFlight';
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
    sections: [
      {
        id: 1,
        label: 'Nord',
        location: null,
        created_at_ms: 1792150700000,
        has_map_photo: false,
        map_photo_at_ms: null,
      },
    ],
    origin: 'web',
    flugmappe_links: [],
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
    origin: 'web',
    flugmappe_links: [],
  },
];

/** Zustandsbehaftete Einsatz-Routen mit den Regeln des Servers (ein laufender Einsatz, 409 beim Starten, 422 beim Löschen eines laufenden). */
export const missionHandlers = (initial: Mission[] = missionFixtures()) => {
  const missions = initial.map((m) => ({ ...m, flugmappe_links: m.flugmappe_links.map((l) => ({ ...l })) }));
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
        origin: 'web',
        flugmappe_links: [],
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
      HttpResponse.json({
        flights: 3,
        photos: 41,
        map_elements: 4,
        video_segments: 2,
      } satisfies MissionContents),
    ),
    http.delete('/api/missions/:id', ({ params }) => {
      const m = find(params.id);
      if (!m) return HttpResponse.json({ error: 'Nicht gefunden' }, { status: 404 });
      if (m.state === 'running')
        return HttpResponse.json(
          { error: 'Ein laufender Einsatz lässt sich nicht löschen' },
          { status: 422 },
        );
      missions.splice(missions.indexOf(m), 1);
      return new HttpResponse(null, { status: 204 });
    }),
    http.post('/api/missions/:id/sections', async ({ params, request }) => {
      const m = find(params.id);
      if (!m) return HttpResponse.json({ error: 'Nicht gefunden' }, { status: 404 });
      if (m.flugmappe_links.some((l) => l.released_at_ms == null))
        return HttpResponse.json(
          { error: 'Abschnitte kommen bei diesem Einsatz aus der Flugmappe' },
          { status: 422 },
        );
      const input = (await request.json()) as SectionCreate;
      m.sections.push({
        id: m.sections.length + 1,
        label: input.label,
        location: input.location,
        created_at_ms: Date.now(),
        has_map_photo: false,
        map_photo_at_ms: null,
      });
      return HttpResponse.json(m);
    }),
    http.post('/api/missions/:id/flugmappe-links/:link/release', ({ params }) => {
      const m = find(params.id);
      const link = m?.flugmappe_links.find((l) => l.id === Number(params.link));
      if (!m || !link) return HttpResponse.json({ error: 'Nicht gefunden' }, { status: 404 });
      link.released_at_ms ??= Date.now();
      return HttpResponse.json(m);
    }),
    // Vorschau-Modus: eine skizzierte Papierkarte statt eines echten Kartenfotos.
    http.get('/api/missions/:id/sections/:section/map-photo', ({ params }) => {
      const s = find(params.id)?.sections.find((x) => x.id === Number(params.section));
      if (!s?.has_map_photo) return HttpResponse.json({ error: 'Nicht gefunden' }, { status: 404 });
      return new HttpResponse(MAP_PHOTO_SVG, { headers: { 'content-type': 'image/svg+xml' } });
    }),
  ];
};

const MAP_PHOTO_SVG =
  '<svg xmlns="http://www.w3.org/2000/svg" width="800" height="600"><rect width="800" height="600" fill="#e8e1cf"/><path d="M60 480 C200 360 320 420 460 300 S700 160 760 120" stroke="#5a6b3a" stroke-width="10" fill="none"/><path d="M220 140 L560 140 L600 420 L180 440 Z" stroke="#c0392b" stroke-width="6" fill="none" stroke-dasharray="18 10"/></svg>';

const noContent = () => new HttpResponse(null, { status: 204 });

/**
 * Standard-Antworten aus den erzeugten Beispieldaten. `me` wählt die Rolle (Tests: Bedienung, Vorschau:
 * Verwaltung), `missions` die Einsätze (die Vorschau hat zusätzlich einen aus der Flugmappe). Die
 * schreibenden Antworten tragen den Vorschau-Modus; Tests, die eine Anfrage prüfen, setzen ihre
 * eigene per `server.use`.
 */
export const handlersFor = (me: Me, missions: Mission[] = missionFixtures()) => {
  // Im Vorschau-Modus gezeichnete Elemente, damit Umbenennen ihre Form behält.
  const drawn = new Map<string, MapElement>(demoElements.map((e) => [e.id, e]));
  const flightMissions = demoFlightMissions();
  return [
    http.get('/api/me', () => HttpResponse.json(me)),
    http.get('/api/health', () => HttpResponse.json(fx.health)),
    http.get('/api/auth/options', () => HttpResponse.json(fx.authOptionsOidc)),
    http.post('/api/auth/logout', noContent),
    http.get('/api/situation', () => HttpResponse.json(fx.snapshotFlying)),
    // Der Einsatz aus der Flugmappe zeigt die Beispiel-Flugliste mit Handeintrag, die übrigen ihre
    // aufgezeichneten Flüge.
    http.get('/api/missions/:id/flight-log', ({ params }) => {
      const id = Number(params.id);
      if (missions.find((m) => m.id === id)?.origin === 'flugmappe') return HttpResponse.json(fx.missionFlights);
      return HttpResponse.json(
        fx.flightList
          .filter((f) => flightMissions.get(f.id) === id)
          .sort((a, b) => a.started_at_ms - b.started_at_ms)
          .map(
            (f): MissionFlight => ({
              key: `t${f.id}`,
              flight_id: f.id,
              source: 'telemetry',
              started_at_ms: f.started_at_ms,
              ended_at_ms: f.ended_at_ms,
              aircraft: 'RC Plus 1',
              section: missions.find((m) => m.id === id)?.sections.at(-1)?.label ?? null,
              confirmed_in: null,
              pilot: null,
            }),
          ),
      );
    }),
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
    // Löschen (nur Verwaltung): Die Vorschau bestätigt nur, ihre Beispieldaten bleiben.
    http.delete('/api/flights/:id', noContent),
    http.delete('/api/flights/:id/video', noContent),
    http.delete('/api/photos/shots/:shot', noContent),
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
    ...missionHandlers(missions),
    http.get('/api/settings', () => HttpResponse.json(fx.settingsView)),
    http.put('/api/settings/:section', () => HttpResponse.json(fx.settingsView)),
    http.delete('/api/settings/flugmappe', () => HttpResponse.json({ ...fx.settingsView, flugmappe: null })),
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
    http.get('/api/accounts', () => HttpResponse.json(fx.accounts)),
    http.put('/api/accounts/:id/block', noContent),
    http.delete('/api/accounts/:id/block', noContent),
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
