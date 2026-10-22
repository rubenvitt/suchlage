import { Space } from 'antd';
import type { ReactNode } from 'react';
import { useMemo, useRef } from 'react';
import { Link, Navigate, useNavigate, useParams } from 'react-router';
import {
  Datenfeld,
  Datenraster,
  Kennzahl,
  Kennzahlenband,
  Paneel,
  PaneelZustand,
  StatusChip,
} from '../components/instrument';
import { ApiError } from '../api/client';
import { useDeleteFlight, useDeleteFlightVideo, useFlight, useFlightPhotos, useMe } from '../api/queries';
import { DeleteButton } from '../components/DeleteButton';
import type { FlightDetail } from '../contract/generated/FlightDetail';
import type { GeoPoint } from '../contract/generated/GeoPoint';
import { useLive, useServerNow } from '../live/LiveProvider';
import { SituationMap } from '../map/SituationMap';
import { LENS_LABEL, mergeShots, shotTitle } from '../photos/photos';
import { usePhotoSelection } from '../photos/usePhotoSelection';
import { Frame } from '../shell/Frame';
import { DASH, fmt } from '../telemetry/format';
import { flightRow } from './flightRow';
import { nearestTrackTime, videoSecond } from './trackVideo';
import './flights.css';

const NO_DEVICES: never[] = [];
const NO_LIVE_SHOTS = {};
// Im Detail zeichnet die Karte keine Live-Zeichen; die Uhr bleibt fest, damit sie nicht sekündlich neu rechnet.
const STATIC_NOW = 0;

/** Nur eine positive ganze Zahl ist eine Flugnummer; alles andere fragt den Server gar nicht erst. */
const flightIdOf = (raw: string) => (/^[1-9]\d*$/.test(raw) ? Number(raw) : null);

function Page({ children }: { children: ReactNode }) {
  return (
    <Frame>
      <div className="sl-page">{children}</div>
    </Frame>
  );
}

export function FlightDetailPage() {
  const raw = useParams().id ?? '';
  const id = flightIdOf(raw);
  const { data, isPending, isError, error, refetch } = useFlight(id);

  // Anderswo gelöscht: Das Neuladen (`missions_changed`) findet den Flug nicht mehr, die Seite geht zur Liste.
  if (data && error instanceof ApiError && error.status === 404) return <Navigate to="/flights" replace />;
  if (id == null || (error instanceof ApiError && error.status === 404)) {
    return (
      <Page>
        <Paneel titel={`Flug ${raw}`} koerperPolster>
          <p role="alert" className="sl-muted">
            Flug {raw} gibt es nicht.{' '}
            <Link className="sl-panel-link" to="/flights">
              Zur Flugliste
            </Link>
          </p>
        </Paneel>
      </Page>
    );
  }
  if (isPending || isError) {
    return (
      <Page>
        <Paneel titel={`Flug ${id}`}>
          <PaneelZustand
            zustand={isPending ? 'laden' : 'fehler'}
            titel={`Flug ${id}`}
            leerText=""
            onNeuladen={() => void refetch()}
          >
            {null}
          </PaneelZustand>
        </Paneel>
      </Page>
    );
  }
  return <Detail data={data} />;
}

function Detail({ data }: { data: FlightDetail }) {
  const f = data.flight;
  const thresholds = useLive((s) => s.thresholds);
  const nowMs = useServerNow();
  // Die Seite rendert sekündlich mit der Uhr; die Karte bekommt die Spur nur neu, wenn neue Daten kommen.
  const tracks = useMemo(() => [data.segments], [data.segments]);
  const { data: listed } = useFlightPhotos(f.id);
  const photos = useMemo(() => mergeShots(listed, NO_LIVE_SHOTS), [listed]);
  const { data: me } = useMe();
  const admin = me?.role === 'admin';
  const photo = usePhotoSelection(photos, me != null && me.role !== 'kiosk', admin);
  const navigate = useNavigate();
  const removeFlight = useDeleteFlight(() => navigate('/flights'));
  const removeVideo = useDeleteFlightVideo();
  const videoRef = useRef<HTMLVideoElement>(null);
  const video = data.video;
  // Klick auf die Spur: Das Video springt an die Stelle, an der das Fluggerät dort war.
  const seekVideo = (p: GeoPoint) => {
    const el = videoRef.current;
    const t = nearestTrackTime(data.segments, p);
    if (!el || !video || t == null) return;
    el.currentTime = videoSecond(t, video.started_at_ms);
    el.scrollIntoView({ block: 'nearest' });
    void el.play().catch(() => {});
  };
  const goes = new Intl.ListFormat('de').format([
    'Spur',
    ...(data.video ? ['Video'] : []),
    ...(photos.length ? [`${photos.length} ${photos.length === 1 ? 'Auslösung' : 'Auslösungen'} mit allen Linsen`] : []),
  ]);
  const r = flightRow(f, nowMs);
  const [distance, distanceUnit] = fmt.distance(f.distance_m);
  const height = fmt.meters(f.max_elevation_m);
  const running = f.ended_at_ms == null;
  return (
    <Frame>
      <div className="sl-detail">
        <div className="sl-detail-map">
          <SituationMap
            gateways={NO_DEVICES}
            aircraft={NO_DEVICES}
            tracks={tracks}
            serverNow={STATIC_NOW}
            thresholds={thresholds}
            staticFit
            label={`Spur von Flug ${f.id}`}
            photos={photos}
            {...photo.mapProps}
            onTrackClick={video ? seekVideo : undefined}
          />
          {photo.overlay}
        </div>
        <div className="sl-detail-side">
          <Paneel
            titel={`Flug ${f.id}`}
            meta={r.date}
            aktion={
              <Space size={12}>
                {/* Ein laufender Flug ist gesperrt (422); der Knopf erscheint erst nach der Landung. */}
                {admin && !running && (
                  <DeleteButton
                    label="Flug löschen …"
                    title={`Flug ${f.id} löschen?`}
                    pending={removeFlight.isPending}
                    error={removeFlight.error}
                    onOpen={removeFlight.reset}
                    onConfirm={() => removeFlight.mutateAsync(f.id)}
                  >
                    Mit dem Flug gelöscht: {goes}.
                  </DeleteButton>
                )}
                <Link className="sl-panel-link" to="/flights">
                  Alle Flüge
                </Link>
              </Space>
            }
          >
            <Kennzahlenband spalten={3} beschriftung="Kennzahlen">
              <Kennzahl groesse="klein" titel={running ? 'Flugzeit' : 'Dauer'} wert={r.duration} />
              <Kennzahl groesse="klein" titel="Strecke" wert={distance} einheit={distanceUnit} />
              <Kennzahl
                groesse="klein"
                titel="max. Höhe"
                wert={height}
                einheit={height === DASH ? undefined : 'm'}
              />
            </Kennzahlenband>
            <Datenraster spalten={2} beschriftung="Flugdaten">
              <Datenfeld label="Ausgang">
                <StatusChip ton={r.outcomeTone} wort={r.outcome} />
              </Datenfeld>
              <Datenfeld label="Start · Ende" mono>
                {r.start} · {r.end}
              </Datenfeld>
              <Datenfeld label="Fernsteuerung" mono>
                {f.gateway_sn}
              </Datenfeld>
              <Datenfeld label="Fluggerät" mono>
                {f.aircraft_sn}
              </Datenfeld>
            </Datenraster>
          </Paneel>
          <Paneel titel="Fotos" meta={photos.length ? `${photos.length} Auslösungen` : undefined} koerperPolster>
            {photos.length ? (
              <div className="sl-photo-strip">
                {photos.map((shot) => {
                  const first = shot.photos[0]!;
                  return (
                    <button
                      key={shot.id}
                      type="button"
                      aria-pressed={photo.mapProps.selectedPhotoId === shot.id}
                      onClick={() => photo.select(shot.id)}
                    >
                      {first.thumb_url ? (
                        <img src={first.thumb_url} alt={`${LENS_LABEL[first.lens]}: ${first.name}`} />
                      ) : (
                        <span className="sl-photo-pending">…</span>
                      )}
                      <span>{shotTitle(shot)}</span>
                    </button>
                  );
                })}
              </div>
            ) : (
              <p className="sl-muted">
                Keine Fotos. Pilot 2 lädt Fotos hoch, sobald der Fotospeicher eingerichtet ist.
              </p>
            )}
          </Paneel>
          <Paneel titel="Video" koerperPolster>
            {data.video ? (
              <div className="sl-detail-video-body">
                <video
                  ref={videoRef}
                  className="sl-detail-video"
                  controls
                  preload="metadata"
                  src={data.video.play_url}
                  aria-label="Flugvideo"
                />
                <a className="sl-panel-link" href={data.video.download_url} download>
                  Video herunterladen
                </a>
                {admin && !running && (
                  <DeleteButton
                    label="Video löschen …"
                    title={`Video von Flug ${f.id} löschen?`}
                    pending={removeVideo.isPending}
                    error={removeVideo.error}
                    onOpen={removeVideo.reset}
                    onConfirm={() => removeVideo.mutateAsync(f.id)}
                  >
                    Gelöscht wird nur die Aufzeichnung; Spur und Fotos bleiben.
                  </DeleteButton>
                )}
                <p className="sl-muted">
                  Das Video beginnt mit dem Livebild, meist kurz nach dem Abheben. Ein Klick auf die Spur springt
                  an diese Stelle im Video.
                </p>
              </div>
            ) : (
              <p className="sl-muted">
                {running
                  ? 'Flug läuft noch – das Video gibt es nach der Landung.'
                  : 'Kein Video aufgezeichnet. Das Livebild lief während dieses Flugs nicht.'}
              </p>
            )}
          </Paneel>
        </div>
      </div>
    </Frame>
  );
}
