import { Button, Input, Select, Switch } from 'antd';
import { Fragment, useEffect, useMemo, useState } from 'react';
import { Paneel, StatusChip } from '../components/instrument';
import type { DeviceLogEntry } from '../contract/generated/DeviceLogEntry';
import type { DeviceLogEvent } from '../contract/generated/DeviceLogEvent';
import { connectLive, type ConnectionStatus } from '../live/connection';
import { useLive } from '../live/LiveProvider';
import {
  applyDeviceLog,
  clock,
  defaultFilter,
  deviceName,
  devices,
  initialDeviceLog,
  prettyPayload,
  SOURCE_LABEL,
  visibleEntries,
  type DeviceLogFilter,
  type DeviceLogState,
} from './deviceLog';

/** Mehr Zeilen rendert die Liste nicht; der Filter holt ältere hervor. */
const SHOWN = 300;

type Connect = typeof connectLive<DeviceLogEvent>;

/** Eigener Strom neben `/api/live`: Nur die Systemseite braucht ihn, und nur Admins dürfen ihn. */
function useDeviceLog(connect: Connect) {
  const [log, setLog] = useState<DeviceLogState>(initialDeviceLog);
  const [status, setStatus] = useState<ConnectionStatus>('connecting');
  useEffect(
    () =>
      connect({
        url: '/api/device-log',
        onEvent: (e) => setLog((s) => applyDeviceLog(s, e)),
        onStatus: setStatus,
      }),
    [connect],
  );
  return { log, status };
}

const STATUS = {
  open: { ton: 'normal', wort: 'live' },
  connecting: { ton: 'neutral', wort: 'verbindet' },
  reconnecting: { ton: 'alarm', wort: 'getrennt – baut neu auf' },
} as const;

interface Props {
  /** Für Tests; sonst die echte EventSource. */
  connect?: Connect;
}

/**
 * Was zwischen Server, Broker und Pilot 2 läuft, live. Zum Fehlersuchen im Betrieb: Befehle und
 * Antworten (über die `tid` verbunden), Topologie, Meldungen der Pilot-Seite.
 */
export function DeviceLogPanel({ connect = connectLive<DeviceLogEvent> }: Props) {
  const { log, status } = useDeviceLog(connect);
  const gateways = useLive((s) => s.gateways);
  const aircraft = useLive((s) => s.aircraft);
  const live = { gateways, aircraft };
  const [filter, setFilter] = useState<DeviceLogFilter>(defaultFilter);
  const [frozen, setFrozen] = useState<DeviceLogEntry[] | null>(null);
  const [open, setOpen] = useState<number | null>(null);
  const source = frozen ?? log.entries;
  const shown = useMemo(() => visibleEntries(source, filter), [source, filter]);
  const waiting = frozen ? log.entries.length - frozen.length : 0;
  const set = (patch: Partial<DeviceLogFilter>) => setFilter((f) => ({ ...f, ...patch }));

  return (
    <Paneel
      titel="Funkprotokoll"
      meta={log.missed > 0 ? `${log.missed} Einträge verpasst` : 'MQTT und Pilot-Seite, live'}
      aktion={<StatusChip {...STATUS[status]} />}
    >
      <div className="sl-devlog-tools">
        <Select
          aria-label="Gerät"
          value={filter.device ?? ''}
          onChange={(v: string) => set({ device: v || null })}
          options={[
            { value: '', label: 'Alle Geräte' },
            ...devices(log.entries).map((sn) => ({ value: sn, label: deviceName(sn, live) })),
          ]}
          style={{ minWidth: 180 }}
        />
        <Input.Search
          aria-label="Suchen"
          placeholder="Methode, Topic, Text"
          allowClear
          onChange={(e) => set({ text: e.target.value })}
          style={{ maxWidth: 240 }}
        />
        <label className="sl-devlog-switch">
          <Switch size="small" checked={filter.onlyProblems} onChange={(v) => set({ onlyProblems: v })} />
          nur Warnungen und Fehler
        </label>
        <label className="sl-devlog-switch">
          <Switch size="small" checked={!filter.hideTelemetry} onChange={(v) => set({ hideTelemetry: !v })} />
          Telemetrie zeigen
        </label>
        <Button onClick={() => setFrozen(frozen ? null : log.entries)}>
          {frozen ? `Weiter${waiting > 0 ? ` (${waiting} neu)` : ''}` : 'Anhalten'}
        </Button>
      </div>
      {shown.length === 0 ? (
        <p className="sl-muted sl-devlog-empty">
          {log.entries.length === 0
            ? 'Noch nichts empfangen. Sobald sich eine Fernsteuerung meldet oder die Pilot-Seite geöffnet wird, erscheint es hier.'
            : 'Kein Eintrag passt zum Filter.'}
        </p>
      ) : (
        <div className="sl-table-scroll">
          <table className="sl-table sl-devlog">
            <thead>
              <tr>
                <th scope="col">Zeit</th>
                <th scope="col">Richtung</th>
                <th scope="col">Gerät</th>
                <th scope="col">Meldung</th>
              </tr>
            </thead>
            <tbody>
              {shown.slice(0, SHOWN).map((e) => (
                <Fragment key={e.seq}>
                  <tr data-level={e.level}>
                    <td className="sl-devlog-time">{clock(e.at_ms)}</td>
                    <td className="sl-devlog-source">{SOURCE_LABEL[e.source]}</td>
                    <td>{deviceName(e.device_sn, live)}</td>
                    <td className="sl-devlog-summary">
                      {e.payload ? (
                        <button
                          type="button"
                          className="sl-devlog-toggle"
                          aria-expanded={open === e.seq}
                          onClick={() => setOpen(open === e.seq ? null : e.seq)}
                        >
                          {e.summary}
                        </button>
                      ) : (
                        e.summary
                      )}
                      {e.topic && <span className="sl-devlog-topic">{e.topic}</span>}
                    </td>
                  </tr>
                  {open === e.seq && e.payload && (
                    <tr className="sl-devlog-payload">
                      <td colSpan={4}>
                        <pre>{prettyPayload(e.payload)}</pre>
                      </td>
                    </tr>
                  )}
                </Fragment>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </Paneel>
  );
}
