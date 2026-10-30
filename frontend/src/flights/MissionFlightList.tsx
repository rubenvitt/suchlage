import { Link } from 'react-router';
import { StatusChip, type StatusTon } from '../components/instrument';
import type { MissionFlight } from '../contract/generated/MissionFlight';
import { DASH, fmt } from '../telemetry/format';
import './flights.css';

const timeOf = (ms: number) => new Date(ms).toLocaleTimeString('de-DE', { hour: '2-digit', minute: '2-digit' });
const dateOf = (ms: number) =>
  new Date(ms).toLocaleDateString('de-DE', { day: '2-digit', month: '2-digit', year: 'numeric' });

/** Woher die Zeile kommt, in Worten (Spec-Nachtrag SUL-84 §4 „Bedienung in Suchlage“). */
export function sourceText(f: MissionFlight): string {
  if (f.source === 'flugmappe') return 'Flugmappe';
  return f.confirmed_in ? `Telemetrie, bestätigt in Flugmappe ${f.confirmed_in}` : 'Telemetrie';
}

const sourceTone = (f: MissionFlight): StatusTon =>
  f.source === 'flugmappe' ? 'neutral' : f.confirmed_in ? 'normal' : 'bedien';

/** Eine Zeile der Flugliste als Anzeigetexte. `nowMs` ist die Serverzeit für Flüge in der Luft. */
export function missionFlightRow(f: MissionFlight, nowMs: number) {
  return {
    date: dateOf(f.started_at_ms),
    start: timeOf(f.started_at_ms),
    end: f.ended_at_ms == null ? 'in der Luft' : timeOf(f.ended_at_ms),
    duration: fmt.duration(((f.ended_at_ms ?? nowMs) - f.started_at_ms) / 1000),
    aircraft: f.aircraft,
    section: f.section ?? DASH,
    source: sourceText(f),
    sourceTone: sourceTone(f),
  };
}

/** Telemetrie-Flüge führen zum Flugdetail mit Spur und Video, Handeinträge haben keine Detailseite. */
function OpenLink({ f, className }: { f: MissionFlight; className: string }) {
  return f.flight_id == null ? null : (
    <Link className={className} to={`/flights/${f.flight_id}`}>
      Flug {f.flight_id} ansehen
    </Link>
  );
}

/** Flugliste eines Einsatzes ab 768 px: Telemetrie und Handeinträge der Flugmappen nach Start. */
export function MissionFlightTable({ flights, nowMs }: { flights: MissionFlight[]; nowMs: number }) {
  return (
    <table className="sl-table">
      <thead>
        <tr>
          <th scope="col">Datum</th>
          <th scope="col">Start</th>
          <th scope="col">Landung</th>
          <th scope="col">Dauer</th>
          <th scope="col">Fluggerät</th>
          <th scope="col">Abschnitt</th>
          <th scope="col">Quelle</th>
          <th scope="col">
            <span className="sl-sr">Öffnen</span>
          </th>
        </tr>
      </thead>
      <tbody>
        {flights.map((f) => {
          const r = missionFlightRow(f, nowMs);
          return (
            <tr key={f.key}>
              <td className="sl-num">{r.date}</td>
              <td className="sl-num">{r.start}</td>
              <td className="sl-num">{r.end}</td>
              <td className="sl-num">{r.duration}</td>
              <td>{r.aircraft}</td>
              <td>{r.section}</td>
              <td>
                <StatusChip ton={r.sourceTone} wort={r.source} />
              </td>
              <td className="sl-flights-open">
                <OpenLink f={f} className="sl-panel-link" />
              </td>
            </tr>
          );
        })}
      </tbody>
    </table>
  );
}

/** Unter 768 px je Flug eine Karte: Zeit und Dauer, Fluggerät und Abschnitt, Quelle und Link. */
export function MissionFlightCards({ flights, nowMs }: { flights: MissionFlight[]; nowMs: number }) {
  return (
    <ul className="sl-cards">
      {flights.map((f) => {
        const r = missionFlightRow(f, nowMs);
        return (
          <li key={f.key} className="sl-card-row">
            <span className="sl-num">
              {r.date} · {r.start}–{r.end}
            </span>
            <span className="sl-num sl-card-duration">{r.duration}</span>
            <span>
              {r.aircraft} · {r.section}
            </span>
            <StatusChip ton={r.sourceTone} wort={r.source} />
            <OpenLink f={f} className="sl-panel-link sl-card-link" />
          </li>
        );
      })}
    </ul>
  );
}
