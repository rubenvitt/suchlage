import { Link } from 'react-router';
import { StatusChip } from '../components/instrument';
import type { FlightSummary } from '../contract/generated/FlightSummary';
import { flightRow } from './flightRow';
import './flights.css';

/** Flugliste ab 768 px. `nowMs` ist die Serverzeit, damit laufende Flüge mitzählen. */
export function FlightTable({ flights, nowMs }: { flights: FlightSummary[]; nowMs: number }) {
  return (
    <table className="sl-table">
      <thead>
        <tr>
          <th scope="col">Datum</th>
          <th scope="col">Start</th>
          <th scope="col">Dauer</th>
          <th scope="col">Strecke</th>
          <th scope="col">max. Höhe</th>
          <th scope="col">Ausgang</th>
          <th scope="col">
            <span className="sl-sr">Öffnen</span>
          </th>
        </tr>
      </thead>
      <tbody>
        {flights.map((f) => {
          const r = flightRow(f, nowMs);
          return (
            <tr key={f.id}>
              <td className="sl-num">{r.date}</td>
              <td className="sl-num">{r.start}</td>
              <td className="sl-num">{r.duration}</td>
              <td className="sl-num">{r.distance}</td>
              <td className="sl-num">{r.maxElevation}</td>
              <td>
                <StatusChip ton={r.outcomeTone} wort={r.outcome} />
              </td>
              <td className="sl-flights-open">
                <Link className="sl-panel-link" to={`/flights/${f.id}`}>
                  Flug {f.id} ansehen
                </Link>
              </td>
            </tr>
          );
        })}
      </tbody>
    </table>
  );
}
