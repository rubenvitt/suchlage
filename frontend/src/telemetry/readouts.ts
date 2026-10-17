import type { AircraftState } from '../contract/generated/AircraftState';
import type { FlightSummary } from '../contract/generated/FlightSummary';
import type { GatewayState } from '../contract/generated/GatewayState';
import type { RtkFix } from '../contract/generated/RtkFix';
import { DASH, fmt } from './format';

export type Tone = 'neutral' | 'normal' | 'achtung' | 'alarm';
export interface Readout {
  key: string;
  label: string;
  value: string;
  unit?: string;
  tone: Tone;
  note?: string;
  /** Der Wert folgt der Uhr, nicht der Telemetrie: sein Ticken ist kein neuer Wert. */
  followsClock?: true;
}

const RTK: Record<RtkFix, [string, Tone]> = {
  fixed: ['RTK fix', 'normal'],
  fixing: ['RTK läuft', 'neutral'],
  not_started: ['ohne RTK', 'neutral'],
  failed: ['RTK gestört', 'alarm'],
};

/** Rückkehr zum Startpunkt, automatische Landung, Zwangslandung, Landung mit drei Rotoren. */
const RETURN_OR_LANDING = [9, 10, 11, 12];

function batteryTone(percent: number | null, returnHome: number | null): Tone {
  if (percent == null || returnHome == null) return 'neutral';
  if (percent <= returnHome) return 'alarm';
  return percent <= returnHome + 10 ? 'achtung' : 'neutral';
}

function linkTone(sdr: number | null): Tone {
  if (sdr == null) return 'neutral';
  if (sdr <= 1) return 'alarm';
  return sdr === 2 ? 'achtung' : 'neutral';
}

/** Laufend gegen die Serveruhr, abgeschlossen bis zum Ende; ohne Flug ein Strich. */
export function flightTime(flight: FlightSummary | undefined, serverNow: number): string {
  if (!flight) return DASH;
  return fmt.duration(((flight.ended_at_ms ?? serverNow) - flight.started_at_ms) / 1000);
}

/** Die Werte der Instrumentenleiste in fester Reihenfolge; fehlende Werte sind ein Strich. */
export function readouts(
  a: AircraftState | undefined,
  g: GatewayState | undefined,
  flight: FlightSummary | undefined,
  serverNow: number,
): Readout[] {
  const t = a?.telemetry;
  const battery = t?.battery;
  const pct = battery?.percent ?? null;
  const gnss = t?.gnss;
  const [rtkWord, rtkTone]: [string, Tone] = gnss?.fix ? RTK[gnss.fix] : [DASH, 'neutral'];
  const sats = [
    gnss?.gps_satellites != null && `${gnss.gps_satellites} GPS`,
    gnss?.rtk_satellites != null && `${gnss.rtk_satellites} RTK`,
  ].filter(Boolean);
  const sdr = g?.link.sdr_quality ?? null;
  const lte = g?.link.lte_quality ?? null;
  const mode = t?.mode;
  const [dist, distUnit] = fmt.distance(t?.home_distance_m);
  const remaining = battery?.remaining_flight_s;

  return [
    {
      key: 'elevation',
      label: 'Höhe ü. Start',
      value: fmt.meters(t?.elevation_takeoff_m),
      unit: 'm',
      tone: 'neutral',
    },
    {
      key: 'speed',
      label: 'Geschw.',
      value: fmt.speed(t?.horizontal_speed_mps),
      unit: 'm/s',
      tone: 'neutral',
    },
    {
      key: 'climb',
      label: 'Steigen',
      value: fmt.signedSpeed(t?.vertical_speed_mps),
      unit: 'm/s',
      tone: 'neutral',
    },
    { key: 'heading', label: 'Kurs', value: fmt.heading(t?.heading_deg), unit: '°', tone: 'neutral' },
    {
      key: 'battery',
      label: 'Akku',
      value: fmt.percent(pct),
      unit: '%',
      tone: batteryTone(pct, battery?.return_home_percent ?? null),
      note: remaining != null ? `${fmt.duration(remaining)} Restflug` : undefined,
    },
    { key: 'distance', label: 'Abstand Start', value: dist, unit: distUnit, tone: 'neutral' },
    {
      key: 'gnss',
      label: 'Positionierung',
      value: rtkWord,
      tone: rtkTone,
      note: sats.length ? sats.join(' · ') : undefined,
    },
    {
      key: 'link',
      label: 'Funk',
      value: sdr == null ? DASH : `${sdr}/5`,
      tone: linkTone(sdr),
      note: lte != null ? `LTE ${lte}/5` : undefined,
    },
    {
      key: 'mode',
      label: 'Modus',
      value: mode?.label ?? DASH,
      tone: mode && RETURN_OR_LANDING.includes(mode.code) ? 'achtung' : 'neutral',
    },
    {
      key: 'flight',
      label: flight?.ended_at_ms != null ? 'Letzter Flug' : 'Flugzeit',
      value: flightTime(flight, serverNow),
      tone: 'neutral',
      followsClock: true,
    },
  ];
}
