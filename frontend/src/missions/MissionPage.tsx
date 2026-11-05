import { Button, Dropdown, type MenuProps } from 'antd';
import { useMemo, useState } from 'react';
import { Link, useNavigate, useParams } from 'react-router';
import { ApiError } from '../api/client';
import {
  useMapPhotos,
  useMe,
  useMissionFlights,
  useMissionSummaries,
  useMissions,
  useStartMission,
} from '../api/queries';
import { useViewMode } from '../app/viewMode';
import {
  Datenfeld,
  Datenraster,
  Kennzahl,
  Kennzahlenband,
  Paneel,
  PaneelZustand,
  StatusChip,
  type PaneelDatenzustand,
} from '../components/instrument';
import type { Mission } from '../contract/generated/Mission';
import { MissionFlightCards, MissionFlightTable } from '../flights/MissionFlightList';
import { useServerNow } from '../live/LiveProvider';
import { shotTitle } from '../photos/photos';
import { Frame } from '../shell/Frame';
import { DeleteMissionDialog } from './DeleteMissionDialog';
import { CloseDialog, EditDialog, FlugmappenDialog, SectionDialog } from './dialogs';
import { activeLinks, FlugmappeLinks, MapPhotoThumb, SECTIONS_FROM_FLUGMAPPE } from './flugmappe';
import { MissionHeader } from './MissionHeader';
import { MissionMarks } from './MissionsPage';
import { dateText, dateTimeText, hoursMinutes, missionDurationMs, STATE_CHIP, timeText } from './missionInfo';
import './missions.css';

const DASH = '–';
/** So viele Vorschaubilder zeigt die Seite; alle stehen auf der Lage. */
const PHOTO_PREVIEW = 24;

type Dialog = 'edit' | 'close' | 'delete' | 'section' | 'flugmappen';

/** Aktionen nach Zustand: die Hauptaktion als Knopf, Seltenes und Endgültiges im Menü „…“. */
function Actions({
  mission,
  otherRunning,
  admin,
  open,
}: {
  mission: Mission;
  otherRunning: boolean;
  admin: boolean;
  open: (d: Dialog) => void;
}) {
  const navigate = useNavigate();
  const start = useStartMission();
  const deletable = admin && mission.state !== 'running' && mission.etb_linked_at_ms == null;
  const menu: NonNullable<MenuProps['items']> = [
    ...(mission.state === 'prepared' ? [{ key: 'edit', label: 'Bearbeiten …' }] : []),
    ...(mission.state === 'running' ? [{ key: 'close', label: 'Einsatz abschließen …', danger: true }] : []),
    ...(deletable ? [{ key: 'delete', label: 'Einsatz löschen …', danger: true }] : []),
  ];
  return (
    <div className="sl-mission-actions">
      {mission.state === 'running' && (
        <>
          <Button type="primary" onClick={() => void navigate('/')}>
            Zur Lage
          </Button>
          <Button onClick={() => open('edit')}>Bearbeiten</Button>
        </>
      )}
      {mission.state === 'prepared' && (
        <Button
          type="primary"
          disabled={otherRunning}
          title={otherRunning ? 'Es läuft schon ein Einsatz' : undefined}
          loading={start.isPending}
          onClick={() => void start.mutateAsync(mission.id).then(() => navigate('/'), () => {})}
        >
          Starten
        </Button>
      )}
      {mission.state !== 'running' && (
        <Button onClick={() => void navigate(`/?einsatz=${mission.id}`)}>
          {mission.state === 'closed' ? 'Auf der Lage ansehen' : 'Auf der Lage vorbereiten'}
        </Button>
      )}
      {menu.length > 0 && (
        <Dropdown trigger={['click']} menu={{ items: menu, onClick: ({ key }) => open(key as Dialog) }}>
          <Button aria-label="Weitere Aktionen">…</Button>
        </Dropdown>
      )}
      {start.error && (
        <span className="sl-mission-action-error" role="alert">
          {start.error instanceof ApiError && start.error.status === 409
            ? 'Es läuft schon ein Einsatz'
            : start.error instanceof ApiError
              ? start.error.message
              : 'Server nicht erreichbar.'}
        </span>
      )}
    </div>
  );
}

function FlightsPanel({ mission }: { mission: Mission }) {
  const flights = useMissionFlights(mission.id);
  const mobile = useViewMode() === 'mobile';
  const nowMs = useServerNow();
  const zustand: PaneelDatenzustand = flights.isPending
    ? 'laden'
    : flights.isError
      ? 'fehler'
      : flights.data.length === 0
        ? 'leer'
        : 'daten';
  return (
    <Paneel
      titel="Flüge"
      className="sl-mission-flights"
      meta={flights.data ? `${flights.data.length} ${flights.data.length === 1 ? 'Flug' : 'Flüge'}` : undefined}
      aktion={
        <Link className="sl-panel-link" to={`/flights?einsatz=${mission.id}`}>
          Flugliste
        </Link>
      }
    >
      <PaneelZustand zustand={zustand} titel="Flüge" leerText="Keine Flüge" onNeuladen={() => void flights.refetch()}>
        {flights.data &&
          (mobile ? (
            <MissionFlightCards flights={flights.data} nowMs={nowMs} />
          ) : (
            <div className="sl-table-scroll">
              <MissionFlightTable flights={flights.data} nowMs={nowMs} />
            </div>
          ))}
      </PaneelZustand>
    </Paneel>
  );
}

function PhotosPanel({ mission }: { mission: Mission }) {
  const scope = useMemo(() => ({ kind: 'mission' as const, mission }), [mission]);
  const shots = useMapPhotos(scope);
  const zustand: PaneelDatenzustand = shots.isPending
    ? 'laden'
    : shots.isError
      ? 'fehler'
      : shots.data.length === 0
        ? 'leer'
        : 'daten';
  // Neueste zuerst: nach dem Einsatz zählt, was zuletzt gefunden wurde.
  const preview = [...(shots.data ?? [])].sort((a, b) => b.taken_at_ms - a.taken_at_ms).slice(0, PHOTO_PREVIEW);
  return (
    <Paneel
      titel="Fotos"
      className="sl-mission-photos"
      meta={shots.data ? `${shots.data.length} Auslösungen` : undefined}
      aktion={
        shots.data && shots.data.length > 0 ? (
          <Link className="sl-panel-link" to={`/?einsatz=${mission.id}`}>
            Auf der Lage
          </Link>
        ) : undefined
      }
    >
      <PaneelZustand zustand={zustand} titel="Fotos" leerText="Keine Fotos" onNeuladen={() => void shots.refetch()}>
        <ul className="sl-photo-grid">
          {preview.map((s) => {
            const photo = s.photos[0];
            return (
              <li key={s.id}>
                {photo ? (
                  <a href={photo.original_url} target="_blank" rel="noreferrer" aria-label={`Foto ${shotTitle(s)} öffnen`}>
                    {photo.thumb_url ? (
                      <img src={photo.thumb_url} alt="" loading="lazy" />
                    ) : (
                      <span className="sl-photo-grid-wait" />
                    )}
                  </a>
                ) : (
                  <span className="sl-photo-grid-wait" />
                )}
                <span className="sl-num">{timeText(s.taken_at_ms)}</span>
              </li>
            );
          })}
        </ul>
      </PaneelZustand>
    </Paneel>
  );
}

function SectionsPanel({ mission, onAdd }: { mission: Mission; onAdd: () => void }) {
  const linked = activeLinks(mission).length > 0;
  return (
    <Paneel
      titel="Abschnitte"
      meta={linked ? SECTIONS_FROM_FLUGMAPPE : String(mission.sections.length)}
      aktion={
        mission.state !== 'closed' && !linked ? (
          <Button size="small" onClick={onAdd}>
            Abschnitt anlegen
          </Button>
        ) : undefined
      }
    >
      {mission.sections.length === 0 ? (
        <p className="sl-mlist-empty">Keine Abschnitte</p>
      ) : (
        <ul className="sl-sections" aria-label={`Abschnitte von ${mission.name}`}>
          {mission.sections.map((s) => (
            <li key={s.id}>
              {s.has_map_photo ? (
                <MapPhotoThumb missionId={mission.id} section={s} />
              ) : (
                <span className="sl-map-photo sl-map-photo-none" aria-hidden="true" />
              )}
              <span>
                <strong>{s.label}</strong>
                <span className="sl-num sl-muted">{timeText(s.created_at_ms)}</span>
              </span>
            </li>
          ))}
        </ul>
      )}
    </Paneel>
  );
}

/**
 * Seite eines Einsatzes (SUL-119), vor allem für die Nachbereitung: Kennzahlen, Flüge, Fotos,
 * Abschnitte, Flugmappen und Verlauf, dazu alle Aktionen am Einsatz.
 */
export function MissionPage() {
  const { id } = useParams();
  const missions = useMissions();
  const summaries = useMissionSummaries();
  const admin = useMe().data?.role === 'admin';
  const mobile = useViewMode() === 'mobile';
  const now = useServerNow(15_000);
  const [dialog, setDialog] = useState<Dialog | null>(null);
  const mission = missions.data?.find((m) => String(m.id) === id);
  const summary = summaries.data?.find((s) => String(s.mission_id) === id);
  const otherRunning = missions.data?.some((m) => m.state === 'running' && m.id !== mission?.id) ?? false;

  if (!mission) {
    return (
      <Frame mission={!mobile && <MissionHeader />}>
        <div className="sl-page">
          {missions.data && (
            <Paneel titel="Einsatz nicht gefunden" koerperPolster>
              <Link className="sl-panel-link" to="/einsaetze">
                Zu den Einsätzen
              </Link>
            </Paneel>
          )}
        </div>
      </Frame>
    );
  }

  const chip = STATE_CHIP[mission.state];
  const duration = missionDurationMs(mission, now);
  const close = () => setDialog(null);
  return (
    <Frame mission={!mobile && <MissionHeader />}>
      <div className="sl-page sl-missions">
        <nav className="sl-crumbs" aria-label="Pfad">
          <Link to="/einsaetze">Einsätze</Link>
          <span aria-hidden="true">/</span>
          <span>{mission.name}</span>
        </nav>

        <div className="sl-title-row">
          <div className="sl-title">
            <div className="sl-title-line">
              <h1 className="sl-h1">{mission.name}</h1>
              <StatusChip ton={chip.ton} wort={chip.wort} />
              <MissionMarks mission={mission} />
            </div>
            <span className="sl-muted">{mission.location ?? 'ohne Ort'}</span>
          </div>
          <Actions mission={mission} otherRunning={otherRunning} admin={admin} open={setDialog} />
        </div>

        <Kennzahlenband beschriftung="Einsatz in Zahlen">
          <Kennzahl
            titel="Beginn"
            wert={mission.started_at_ms != null ? timeText(mission.started_at_ms) : DASH}
            notiz={mission.started_at_ms != null ? dateText(mission.started_at_ms) : 'nicht gestartet'}
          />
          <Kennzahl
            titel={mission.state === 'running' ? 'Läuft seit' : 'Dauer'}
            wert={duration != null ? hoursMinutes(duration) : DASH}
            einheit={duration != null ? 'h' : undefined}
            punkt={mission.state === 'running' ? 'normal' : undefined}
            notiz={mission.closed_at_ms != null ? `Ende ${timeText(mission.closed_at_ms)}` : undefined}
          />
          <Kennzahl
            titel="Flüge"
            wert={summary?.flights ?? DASH}
            notiz={summary ? `${hoursMinutes(summary.flight_ms)} h Flugzeit` : undefined}
          />
          <Kennzahl titel="Fotos" wert={summary?.photos ?? DASH} />
          <Kennzahl titel="Kartenelemente" wert={summary?.map_elements ?? DASH} />
        </Kennzahlenband>

        <div className="sl-mission-grid">
          <FlightsPanel mission={mission} />
          <PhotosPanel mission={mission} />
          <div className="sl-mission-side">
            <SectionsPanel mission={mission} onAdd={() => setDialog('section')} />
            {mission.flugmappe_links.length > 0 && (
              <Paneel
                titel="Flugmappen"
                meta={`${mission.flugmappe_links.length} verknüpft`}
                aktion={
                  mission.state !== 'closed' && activeLinks(mission).length > 0 ? (
                    <Button size="small" onClick={() => setDialog('flugmappen')}>
                      Lösen …
                    </Button>
                  ) : undefined
                }
              >
                <FlugmappeLinks mission={mission} />
              </Paneel>
            )}
            <Paneel titel="Verlauf">
              <Datenraster spalten={2} beschriftung="Verlauf">
                <Datenfeld label="Angelegt" mono>
                  {dateTimeText(mission.created_at_ms)}
                </Datenfeld>
                <Datenfeld label="von">{mission.created_by}</Datenfeld>
                <Datenfeld label="Gestartet" mono>
                  {mission.started_at_ms != null ? dateTimeText(mission.started_at_ms) : DASH}
                </Datenfeld>
                <Datenfeld label="Abgeschlossen" mono>
                  {mission.closed_at_ms != null ? dateTimeText(mission.closed_at_ms) : DASH}
                </Datenfeld>
                <Datenfeld label="Einsatztagebuch" breit>
                  {mission.etb_linked_at_ms != null ? (
                    <StatusChip ton="bedien" wort={`verknüpft ${dateTimeText(mission.etb_linked_at_ms)}`} />
                  ) : (
                    <span className="sl-muted">nicht verknüpft</span>
                  )}
                </Datenfeld>
              </Datenraster>
            </Paneel>
          </div>
        </div>
      </div>
      {dialog === 'edit' && <EditDialog mission={mission} onClose={close} />}
      {dialog === 'section' && <SectionDialog mission={mission} onClose={close} />}
      {dialog === 'flugmappen' && <FlugmappenDialog mission={mission} onClose={close} />}
      {dialog === 'close' && <CloseDialog mission={mission} onClose={close} onDone={close} />}
      {dialog === 'delete' && <DeleteMissionDialog mission={mission} onClose={close} />}
    </Frame>
  );
}
