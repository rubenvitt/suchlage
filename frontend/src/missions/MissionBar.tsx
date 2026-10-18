import { Select, Tag } from 'antd';
import { useMissions } from '../api/queries';
import type { MissionState } from '../contract/generated/MissionState';
import './missions.css';
import { useMissionScope } from './useMissionScope';

const STATE_LABEL: Record<MissionState, string> = {
  prepared: 'Vorbereitet',
  running: 'Laufend',
  closed: 'Abgeschlossen',
};
const STATE_COLOR: Record<MissionState, string> = {
  prepared: 'default',
  running: 'green',
  closed: 'default',
};
const ORDER: MissionState[] = ['running', 'prepared', 'closed'];

/** Kopfleiste mit dem gewählten Einsatz, seinem Zustand, dem aktiven Abschnitt und der Auswahl. */
export function MissionBar() {
  const { data: missions = [], isError } = useMissions();
  const { scope, setScope, ready } = useMissionScope();
  // Ohne bekannte Einsätze keine Auswahl: „ohne Einsatz“ wäre hier geraten.
  if (!ready)
    return (
      <div className="sl-mission-bar">
        <span>{isError ? 'Einsätze nicht geladen' : 'Einsätze werden geladen …'}</span>
      </div>
    );
  const options = [
    ...ORDER.filter((s) => s !== 'closed').flatMap((s) =>
      missions.filter((m) => m.state === s).map((m) => ({ value: String(m.id), label: m.name })),
    ),
    { value: 'ohne', label: 'ohne Einsatz' },
    ...missions
      .filter((m) => m.state === 'closed')
      .map((m) => ({ value: String(m.id), label: `${m.name} (abgeschlossen)` })),
  ];
  const section = scope.kind === 'mission' ? scope.mission.sections.at(-1) : undefined;
  return (
    <div className="sl-mission-bar">
      {scope.kind === 'mission' ? (
        <>
          <strong>{scope.mission.name}</strong>
          <Tag color={STATE_COLOR[scope.mission.state]}>{STATE_LABEL[scope.mission.state]}</Tag>
          {section && <span>Abschnitt {section.label}</span>}
        </>
      ) : (
        <strong>ohne Einsatz</strong>
      )}
      <Select
        aria-label="Einsatz wählen"
        size="small"
        className="sl-mission-select"
        value={scope.kind === 'mission' ? String(scope.mission.id) : 'ohne'}
        options={options}
        onChange={setScope}
      />
    </div>
  );
}
