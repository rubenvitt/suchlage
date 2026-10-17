import type { DeviceLogEntry } from '../contract/generated/DeviceLogEntry';
import type { DeviceLogEvent } from '../contract/generated/DeviceLogEvent';
import type { DeviceLogSource } from '../contract/generated/DeviceLogSource';
import type { LiveState } from '../live/reducer';

/** So viele Einträge hält der Browser; ältere fallen heraus. */
export const KEEP = 1_000;

export interface DeviceLogState {
  /** Älteste zuerst. */
  entries: DeviceLogEntry[];
  /** Einträge, die der Strom nicht mitbekommen hat (Lücken in `seq` oder `missed`). */
  missed: number;
}

export const initialDeviceLog: DeviceLogState = { entries: [], missed: 0 };

export function applyDeviceLog(state: DeviceLogState, event: DeviceLogEvent): DeviceLogState {
  switch (event.type) {
    case 'history':
      // Nach einem Neuverbinden ersetzt der Verlauf alles: Er reicht weiter zurück als jede Lücke.
      return { entries: event.entries.slice(-KEEP), missed: state.missed };
    case 'entry': {
      const last = state.entries.at(-1);
      if (last && event.entry.seq <= last.seq) return state;
      const gap = last ? event.entry.seq - last.seq - 1 : 0;
      return {
        entries: [...state.entries, event.entry].slice(-KEEP),
        missed: state.missed + gap,
      };
    }
    case 'missed':
      return { ...state, missed: state.missed + event.count };
  }
}

export interface DeviceLogFilter {
  /** Seriennummer oder `null` für alle. */
  device: string | null;
  hideTelemetry: boolean;
  onlyProblems: boolean;
  text: string;
}

export const defaultFilter: DeviceLogFilter = { device: null, hideTelemetry: true, onlyProblems: false, text: '' };

const isTelemetry = (e: DeviceLogEntry) => e.topic?.endsWith('/osd') ?? false;

/** Neueste zuerst, wie man ein laufendes Protokoll liest. */
export function visibleEntries(entries: DeviceLogEntry[], f: DeviceLogFilter): DeviceLogEntry[] {
  const needle = f.text.trim().toLowerCase();
  return entries
    .filter(
      (e) =>
        (f.device === null || e.device_sn === f.device) &&
        !(f.hideTelemetry && isTelemetry(e)) &&
        !(f.onlyProblems && e.level === 'info') &&
        (!needle ||
          [e.summary, e.method, e.topic].some((v) => v?.toLowerCase().includes(needle))),
    )
    .reverse();
}

export const SOURCE_LABEL: Record<DeviceLogSource, string> = {
  device: 'Gerät → Server',
  server: 'Server → Gerät',
  pilot: 'Pilot-Seite',
  system: 'Server',
};

/** „RC Plus 1“ für die Fernsteuerung, „RC Plus 1 · Fluggerät“ für ihr Fluggerät, sonst die SN. */
export function deviceName(sn: string | null, live: Pick<LiveState, 'gateways' | 'aircraft'>): string {
  if (!sn) return '–';
  const gateway = live.gateways[sn];
  if (gateway) return gateway.label ?? sn;
  const aircraft = live.aircraft[sn];
  if (aircraft) {
    const label = live.gateways[aircraft.gateway_sn]?.label;
    return label ? `${label} · Fluggerät` : sn;
  }
  return sn;
}

/** Alle Seriennummern im Protokoll, für die Auswahl. */
export function devices(entries: DeviceLogEntry[]): string[] {
  return [...new Set(entries.flatMap((e) => (e.device_sn ? [e.device_sn] : [])))].sort();
}

/** `14:23:05.120` in Ortszeit. */
export function clock(ms: number): string {
  const d = new Date(ms);
  const two = (n: number) => String(n).padStart(2, '0');
  return `${two(d.getHours())}:${two(d.getMinutes())}:${two(d.getSeconds())}.${String(d.getMilliseconds()).padStart(3, '0')}`;
}

/** JSON eingerückt; gekürzte oder kaputte Payloads bleiben wie sie sind. */
export function prettyPayload(payload: string): string {
  try {
    return JSON.stringify(JSON.parse(payload), null, 2);
  } catch {
    return payload;
  }
}
