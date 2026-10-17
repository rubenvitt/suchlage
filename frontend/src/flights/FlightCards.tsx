import { Link } from 'react-router';
import { StatusChip } from '../components/instrument';
import type { FlightSummary } from '../contract/generated/FlightSummary';
import { flightRow } from './flightRow';
import './flights.css';

/** Flugliste unter 768 px: Sieben Spalten passen nicht auf 360 px, je Flug eine Karte mit dem Wichtigsten. */
export function FlightCards({ flights, nowMs }: { flights: FlightSummary[]; nowMs: number }) {
  return (
    <ul className="sl-cards">
      {flights.map((f) => {
        const r = flightRow(f, nowMs);
        return (
          <li key={f.id} className="sl-card-row">
            <span className="sl-num">
              {r.date} · {r.start}
            </span>
            <span className="sl-num sl-card-duration">{r.duration}</span>
            <StatusChip ton={r.outcomeTone} wort={r.outcome} />
            <Link className="sl-panel-link sl-card-link" to={`/flights/${f.id}`}>
              Flug {f.id} ansehen
            </Link>
          </li>
        );
      })}
    </ul>
  );
}
