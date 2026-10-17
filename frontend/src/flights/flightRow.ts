import type { StatusTon } from '../components/instrument';
import type { FlightEndReason } from '../contract/generated/FlightEndReason';
import type { FlightSummary } from '../contract/generated/FlightSummary';
import { DASH, fmt } from '../telemetry/format';
import { flightTime } from '../telemetry/readouts';

interface Outcome {
  word: string;
  tone: StatusTon;
}

const ENDED_BY: Record<FlightEndReason, Outcome> = {
  landed: { word: 'gelandet', tone: 'normal' },
  // Ein Flug ohne Landemeldung ist kein Alarm, aber zu prüfen: Wo ist das Fluggerät?
  connection_lost: { word: 'Verbindung verloren', tone: 'achtung' },
};
const RUNNING: Outcome = { word: 'läuft', tone: 'bedien' };
const ENDED: Outcome = { word: 'beendet', tone: 'neutral' };

const dateOf = (ms: number) =>
  new Date(ms).toLocaleDateString('de-DE', { day: '2-digit', month: '2-digit', year: 'numeric' });
const timeOf = (ms: number) =>
  new Date(ms).toLocaleTimeString('de-DE', { hour: '2-digit', minute: '2-digit' });

function outcomeOf(f: FlightSummary): Outcome {
  if (f.ended_at_ms == null) return RUNNING;
  return f.end_reason ? ENDED_BY[f.end_reason] : ENDED;
}

/** Ein Flug als Anzeigetexte für Liste, Karte und Detail. `nowMs` ist die Serverzeit für laufende Flüge. */
export function flightRow(f: FlightSummary, nowMs: number) {
  const [distance, unit] = fmt.distance(f.distance_m);
  const outcome = outcomeOf(f);
  return {
    date: dateOf(f.started_at_ms),
    start: timeOf(f.started_at_ms),
    end: f.ended_at_ms == null ? DASH : timeOf(f.ended_at_ms),
    duration: flightTime(f, nowMs),
    distance: `${distance} ${unit}`,
    maxElevation: f.max_elevation_m == null ? DASH : `${fmt.meters(f.max_elevation_m)} m`,
    outcome: outcome.word,
    outcomeTone: outcome.tone,
  };
}
