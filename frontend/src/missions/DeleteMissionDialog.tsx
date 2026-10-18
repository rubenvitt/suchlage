import { Alert, Input, Modal } from 'antd';
import { useState } from 'react';
import { useNavigate } from 'react-router';
import { ApiError } from '../api/client';
import { useDeleteMission, useMissionContents } from '../api/queries';
import type { Mission } from '../contract/generated/Mission';
import type { MissionContents } from '../contract/generated/MissionContents';

const errorText = (e: unknown) => (e instanceof ApiError ? e.message : 'Server nicht erreichbar.');
const count = (n: number, one: string, many: string) => `${n} ${n === 1 ? one : many}`;

/** „3 Flüge, 2 Videosegmente, 41 Fotos, 4 Kartenelemente“ */
const contentsText = (c: MissionContents) =>
  [
    count(c.flights, 'Flug', 'Flüge'),
    count(c.video_segments, 'Videosegment', 'Videosegmente'),
    count(c.photos, 'Foto', 'Fotos'),
    count(c.map_elements, 'Kartenelement', 'Kartenelemente'),
  ].join(', ');

/**
 * Rückfrage vor dem Löschen eines Einsatzes: nennt, was mitgeht, und gibt erst frei, wenn der Name
 * eingetippt ist. Danach geht es zur Einsatzübersicht.
 */
export function DeleteMissionDialog({ mission, onClose }: { mission: Mission; onClose: () => void }) {
  const navigate = useNavigate();
  const contents = useMissionContents(mission.id);
  const remove = useDeleteMission(() => navigate('/einsaetze'));
  const [typed, setTyped] = useState('');
  return (
    <Modal
      open
      title={`Einsatz „${mission.name}“ löschen?`}
      okText="Endgültig löschen"
      cancelText="Abbrechen"
      okButtonProps={{ danger: true, disabled: !contents.data || typed !== mission.name }}
      confirmLoading={remove.isPending}
      onOk={() => remove.mutate(mission.id)}
      onCancel={onClose}
    >
      {contents.isPending && <p>Inhalt wird gezählt …</p>}
      {contents.isError && <Alert type="error" title={errorText(contents.error)} />}
      {contents.data && (
        <p>
          Gelöscht werden {contentsText(contents.data)} und die Abschnitte. Das lässt sich nicht rückgängig
          machen.
        </p>
      )}
      <label className="sl-confirm-name">
        Zum Bestätigen den Einsatznamen eintippen
        <Input value={typed} onChange={(e) => setTyped(e.target.value)} autoComplete="off" />
      </label>
      {remove.isError && <Alert type="error" title={errorText(remove.error)} />}
    </Modal>
  );
}
