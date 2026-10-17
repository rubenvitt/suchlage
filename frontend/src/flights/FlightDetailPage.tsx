import type { ReactNode } from 'react';
import { useMemo } from 'react';
import { Link, useParams } from 'react-router';
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
import { useFlight } from '../api/queries';
import type { FlightDetail } from '../contract/generated/FlightDetail';
import { useLive, useServerNow } from '../live/LiveProvider';
import { SituationMap } from '../map/SituationMap';
import { Frame } from '../shell/Frame';
import { DASH, fmt } from '../telemetry/format';
import { flightRow } from './flightRow';
import './flights.css';

const NO_DEVICES: never[] = [];
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
          />
        </div>
        <div className="sl-detail-side">
          <Paneel
            titel={`Flug ${f.id}`}
            meta={r.date}
            aktion={
              <Link className="sl-panel-link" to="/flights">
                Alle Flüge
              </Link>
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
          <Paneel titel="Video" koerperPolster>
            {data.video ? (
              <div className="sl-detail-video-body">
                <video
                  className="sl-detail-video"
                  controls
                  preload="metadata"
                  src={data.video.play_url}
                  aria-label="Flugvideo"
                />
                <a className="sl-panel-link" href={data.video.download_url} download>
                  Video herunterladen
                </a>
                <p className="sl-muted">Das Video beginnt mit dem Livebild, meist kurz nach dem Abheben.</p>
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
