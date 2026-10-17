import { useState } from 'react';
import type { AircraftState } from '../contract/generated/AircraftState';
import type { FlightSummary } from '../contract/generated/FlightSummary';
import type { FreshnessThresholds } from '../contract/generated/FreshnessThresholds';
import type { GatewayState } from '../contract/generated/GatewayState';
import { ageLabel, freshnessOf, type Freshness } from '../live/freshness';
import { DASH, fmt } from './format';
import { readouts, type Tone } from './readouts';
import './strip.css';

interface Props {
  aircraft?: AircraftState;
  gateway?: GatewayState;
  flight?: FlightSummary;
  serverNow: number;
  thresholds: FreshnessThresholds;
  size: 'compact' | 'large' | 'mobile';
  /** Mobil zeigt nur die wichtigsten Werte; das Alter steht immer da. */
  only?: string[];
}

const ANNOUNCEMENT: Record<Freshness, string> = {
  fresh: 'Telemetrie aktuell',
  delayed: 'Telemetrie verzögert',
  lost: 'Telemetrie verloren',
};

function ageCell(aircraft: AircraftState | undefined, freshness: Freshness, age: number) {
  if (!aircraft) return { value: DASH, tone: 'neutral' as Tone };
  if (freshness === 'fresh')
    return { value: `${Math.max(1, Math.round(age / 1000))} s`, tone: 'neutral' as Tone };
  if (freshness === 'delayed') return { value: ageLabel(age), tone: 'achtung' as Tone };
  return { value: `Verbindung verloren ${fmt.clock(aircraft.last_seen_ms)}`, tone: 'alarm' as Tone };
}

/**
 * Ein neuer Wert ist ein neues Element und spielt damit die 120-ms-Färbung genau einmal ab. Das erste
 * Bild bleibt ruhig: Erscheinen ist kein neuer Wert.
 */
function FlashValue({ value }: { value: string }) {
  const [shown, setShown] = useState(value);
  const [changes, setChanges] = useState(0);
  if (value !== shown) {
    setShown(value);
    setChanges(changes + 1);
  }
  return (
    <span key={changes} className={changes > 0 ? 'sl-strip-flash' : undefined}>
      {value}
    </span>
  );
}

export function TelemetryStrip({ aircraft, gateway, flight, serverNow, thresholds, size, only }: Props) {
  const freshness = aircraft ? freshnessOf(aircraft.last_seen_ms, serverNow, thresholds) : 'lost';
  const age = ageCell(aircraft, freshness, aircraft ? serverNow - aircraft.last_seen_ms : 0);
  const hatch =
    !aircraft || freshness === 'fresh' ? '' : freshness === 'delayed' ? 'sl-hatch-delayed' : 'sl-hatch-lost';
  const items = readouts(aircraft, gateway, flight, serverNow).filter((r) => !only || only.includes(r.key));

  return (
    <section
      className={`sl-strip ${hatch}`}
      data-freshness={freshness}
      data-size={size}
      data-fugenraster=""
      aria-label="Telemetrie"
    >
      {items.map((r) => (
        <div key={r.key} className="sl-strip-cell" data-key={r.key} data-tone={r.tone}>
          <span className="sl-strip-label">{r.label}</span>
          <span className="sl-strip-value" title={r.value}>
            {r.followsClock ? r.value : <FlashValue value={r.value} />}
            {r.unit && r.value !== DASH && <span className="sl-strip-unit">{r.unit}</span>}
          </span>
          {r.note && <span className="sl-strip-note">{r.note}</span>}
        </div>
      ))}
      <div className="sl-strip-cell sl-strip-age" data-tone={age.tone}>
        <span className="sl-strip-label">Alter</span>
        <span className="sl-strip-value">{age.value}</span>
        {/* Vorgelesen wird nur der Wechsel der Frische; das Alter selbst tickt jede Sekunde. */}
        <span className="sl-sr" role="status">
          {aircraft ? ANNOUNCEMENT[freshness] : ''}
        </span>
      </div>
    </section>
  );
}
