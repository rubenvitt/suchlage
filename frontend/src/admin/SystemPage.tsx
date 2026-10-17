import { Datenfeld, Datenraster, Paneel, StatusChip, type StatusTon } from '../components/instrument';
import type { BrokerStatus } from '../contract/generated/BrokerStatus';
import type { ConnectionStatus } from '../live/connection';
import { useConnectionStatus, useLive } from '../live/LiveProvider';
import { Frame } from '../shell/Frame';
import './admin.css';

type Chip = { ton: StatusTon; wort: string };

const BROKER: Record<BrokerStatus, Chip> = {
  connected: { ton: 'normal', wort: 'verbunden' },
  connecting: { ton: 'neutral', wort: 'verbindet' },
  disconnected: { ton: 'alarm', wort: 'getrennt' },
  not_configured: { ton: 'achtung', wort: 'nicht eingerichtet' },
};

const LIVE: Record<ConnectionStatus, Chip> = {
  open: { ton: 'normal', wort: 'verbunden' },
  connecting: { ton: 'neutral', wort: 'verbindet' },
  reconnecting: { ton: 'alarm', wort: 'getrennt – baut neu auf' },
};

/** Ohne `system` hat der Server noch keinen Stand geschickt: Dann steht nichts da, was nach Lage aussieht. */
export function SystemView() {
  const system = useLive((s) => s.system);
  const status = useConnectionStatus();
  return (
    <div className="sl-page sl-system">
      <Paneel titel="Verbindungen">
        <Datenraster spalten={4} beschriftung="Verbindungen">
          <Datenfeld label="Broker">
            <StatusChip {...(system ? BROKER[system.broker] : { ton: 'neutral', wort: 'unbekannt' })} />
          </Datenfeld>
          <Datenfeld label="Live-Strom zum Browser">
            <StatusChip {...LIVE[status]} />
          </Datenfeld>
          <Datenfeld label="Rohmitschnitt" mono>
            {system ? (system.raw_recording ?? 'aus') : '–'}
          </Datenfeld>
          <Datenfeld label="Nicht lesbare Nachrichten" mono>
            {system ? system.invalid_messages : '–'}
          </Datenfeld>
        </Datenraster>
      </Paneel>
      <Paneel
        titel="Nicht ausgewertete DJI-Felder"
        meta="nach Pilot-2- oder Firmware-Updates prüfen"
        koerperPolster={!system?.unknown_fields.length}
      >
        {!system ? (
          <p className="sl-muted">Noch kein Stand vom Server.</p>
        ) : system.unknown_fields.length > 0 ? (
          <table className="sl-table">
            <thead>
              <tr>
                <th scope="col">Feld</th>
                <th scope="col" className="sl-col-count">
                  Anzahl
                </th>
              </tr>
            </thead>
            <tbody>
              {system.unknown_fields.map((u) => (
                <tr key={u.key}>
                  <td>
                    <code className="sl-code">{u.key}</code>
                  </td>
                  <td className="sl-num sl-col-count">{u.count}</td>
                </tr>
              ))}
            </tbody>
          </table>
        ) : (
          <p className="sl-muted">Alle empfangenen Felder sind bekannt.</p>
        )}
      </Paneel>
    </div>
  );
}

/** Der Live-Strom kommt aus `SessionLayout` (F2), wie auf der Lage-Seite. */
export function SystemPage() {
  return (
    <Frame>
      <SystemView />
    </Frame>
  );
}
