import { Link } from 'react-router';
import { Kennzahl, Kennzahlenband, Paneel } from '../components/instrument';
import { DASH, fmt } from '../telemetry/format';
import { flightTime } from '../telemetry/readouts';
import type { Focus } from './selectors';

interface Props {
  focus: Focus;
  serverNow: number;
  /** Großbild und mobil haben keine Flugliste. */
  showLink?: boolean;
}

function titleOf({ flight, aircraft }: Focus): string {
  if (flight) return 'Flug läuft';
  if (aircraft?.phase === 'airborne') return 'In der Luft';
  if (aircraft?.phase === 'ground') return 'Am Boden';
  return 'Flug';
}

/** Der laufende Flug, sonst der zuletzt beendete des Fluggeräts. */
export function FlightPanel({ focus, serverNow, showLink = true }: Props) {
  const running = focus.flight?.flight;
  const last = running ? undefined : focus.ended;
  const shown = running ?? last;
  const [distance, distanceUnit] = fmt.distance(shown?.distance_m);
  const height = fmt.meters(shown?.max_elevation_m);
  return (
    <Paneel
      titel={titleOf(focus)}
      meta={last?.ended_at_ms != null ? `beendet ${fmt.clock(last.ended_at_ms)}` : undefined}
      aktion={
        showLink ? (
          <Link className="sl-panel-link" to="/flights">
            Alle Flüge
          </Link>
        ) : undefined
      }
      koerperPolster={!shown}
    >
      {shown ? (
        <Kennzahlenband spalten={3} beschriftung={running ? 'Laufender Flug' : 'Letzter Flug'}>
          <Kennzahl
            groesse="klein"
            titel={running ? 'Flugzeit' : 'Dauer'}
            wert={flightTime(shown, serverNow)}
          />
          <Kennzahl groesse="klein" titel="Strecke" wert={distance} einheit={distanceUnit} />
          <Kennzahl
            groesse="klein"
            titel="max. Höhe"
            wert={height}
            einheit={height === DASH ? undefined : 'm'}
          />
        </Kennzahlenband>
      ) : (
        <p className="sl-muted">
          {focus.aircraft
            ? 'Noch kein Flug. Er beginnt automatisch beim Abheben.'
            : 'Kein Fluggerät verbunden. Ein Flug beginnt automatisch beim Abheben.'}
        </p>
      )}
    </Paneel>
  );
}
