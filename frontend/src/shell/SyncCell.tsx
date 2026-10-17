import { StatusChip, type StatusTon } from '../components/instrument';
import type { BrokerStatus } from '../contract/generated/BrokerStatus';
import { useConnectionStatus, useLive } from '../live/LiveProvider';

type Chip = { ton: StatusTon; wort: string; title: string };

/** Steht der Broker, zählt nur der Live-Strom; sonst sagt der Kopf, warum keine Telemetrie kommt. */
const BROKER: Record<BrokerStatus, Chip | null> = {
  connected: null,
  connecting: { ton: 'neutral', wort: 'Broker verbindet', title: 'Der Server verbindet sich mit dem Broker' },
  disconnected: {
    ton: 'alarm',
    wort: 'Broker getrennt',
    title: 'Der Server erreicht den Broker nicht – es kommt keine Telemetrie an',
  },
  not_configured: {
    ton: 'achtung',
    wort: 'Broker nicht eingerichtet',
    title: 'Die Verwaltung trägt den Broker unter Einstellungen → Broker ein',
  },
};

/** Live-Strom und Broker als ein Wort mit Ton. */
export function SyncCell() {
  const status = useConnectionStatus();
  const broker = useLive((s) => s.system?.broker ?? null);
  if (status === 'reconnecting')
    return (
      <StatusChip
        ton="alarm"
        wort="GETRENNT"
        title="Live-Verbindung zum Server unterbrochen – baut neu auf"
      />
    );
  if (status === 'connecting')
    return <StatusChip ton="neutral" wort="verbindet" title="Live-Verbindung wird aufgebaut" />;
  const chip = broker ? BROKER[broker] : null;
  if (chip) return <StatusChip {...chip} />;
  return <StatusChip ton="normal" wort="SYNC" title="Live-Verbindung steht" />;
}
