import { Select } from 'antd';
import { useMissions } from '../api/queries';

/** „Einsatz zuordnen“ für Flüge und Elemente ohne Einsatz: Auswahl unter allen Einsätzen, auch abgeschlossenen. */
export function AssignMissionSelect({
  onAssign,
  busy,
}: {
  onAssign: (missionId: number) => void;
  busy: boolean;
}) {
  const { data: missions = [] } = useMissions();
  return (
    <Select
      aria-label="Einsatz zuordnen"
      placeholder="Einsatz zuordnen"
      size="small"
      className="sl-mission-select"
      loading={busy}
      disabled={busy}
      value={null}
      options={missions.map((m) => ({
        value: m.id,
        label: m.state === 'closed' ? `${m.name} (abgeschlossen)` : m.name,
      }))}
      onChange={onAssign}
    />
  );
}
