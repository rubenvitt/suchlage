import { Alert, Button, Form, Input, Modal } from 'antd';
import { useState } from 'react';
import { useNavigate } from 'react-router';
import { ApiError } from '../api/client';
import { useAddSection, useCloseMission, useReleaseFlugmappe, useUpdateMission } from '../api/queries';
import type { FlugmappeLink } from '../contract/generated/FlugmappeLink';
import type { Mission } from '../contract/generated/Mission';
import { useSkip } from './entry';
import { FlugmappeLinks, MapPhotoThumb } from './flugmappe';
import './missions.css';

/* Dialoge am Einsatz: aus dem Kopfleisten-Menü des laufenden Einsatzes und von der Einsatzseite (SUL-119). */

const errorText = (e: unknown) => (e instanceof ApiError ? e.message : 'Server nicht erreichbar.');

export function SectionDialog({ mission, onClose }: { mission: Mission; onClose: () => void }) {
  const add = useAddSection();
  const [form] = Form.useForm<{ label: string }>();
  return (
    <Modal
      open
      title="Neuer Abschnitt"
      okText="Anlegen"
      cancelText="Abbrechen"
      confirmLoading={add.isPending}
      onOk={() => form.submit()}
      onCancel={onClose}
    >
      <Form
        form={form}
        layout="vertical"
        requiredMark={false}
        onFinish={(v) =>
          void add.mutateAsync({ id: mission.id, label: v.label, location: null }).then(onClose, () => {})
        }
      >
        <Form.Item label="Abschnitt" name="label" rules={[{ required: true, message: 'Name fehlt' }]}>
          <Input autoFocus placeholder="z. B. Süd" />
        </Form.Item>
      </Form>
      {add.isError && <Alert type="error" title={errorText(add.error)} />}
    </Modal>
  );
}

/**
 * Verknüpfte Flugmappen mit Zustand. „Lösen“ ist der Ausweg bei einem verlorenen Gerät: Die Flugmappe
 * darf danach nichts mehr am Einsatz, bis sie neu verknüpft; ihre Flugbucheinträge bleiben.
 */
export function FlugmappenDialog({ mission, onClose }: { mission: Mission; onClose: () => void }) {
  const release = useReleaseFlugmappe();
  const [asking, setAsking] = useState<FlugmappeLink | null>(null);
  return (
    <Modal open title={`Flugmappen · ${mission.name}`} footer={null} onCancel={onClose}>
      <p className="sl-mission-fm-sections">
        Der Einsatz schließt aus der Flugmappe, wenn keine verknüpfte mehr offen ist.
      </p>
      <FlugmappeLinks
        mission={mission}
        onRelease={setAsking}
        releasing={release.isPending ? (release.variables?.linkId ?? null) : null}
      />
      {asking && (
        <Alert
          type="warning"
          title={`Flugmappe ${asking.aircraft_label} lösen?`}
          description="Sie kann danach nichts mehr an diesem Einsatz ändern, bis sie neu verknüpft. Gemeldete Flüge bleiben."
          action={
            <span className="sl-fm-release-actions">
              <Button size="small" onClick={() => setAsking(null)}>
                Abbrechen
              </Button>
              <Button
                size="small"
                danger
                loading={release.isPending}
                onClick={() =>
                  void release.mutateAsync({ id: mission.id, linkId: asking.id }).then(
                    () => setAsking(null),
                    () => {},
                  )
                }
              >
                Lösen
              </Button>
            </span>
          }
        />
      )}
      {release.isError && <Alert type="error" title={errorText(release.error)} />}
    </Modal>
  );
}

/** Kartenfotos der Abschnitte (aus der Flugmappe), neueste Abschnitte zuletzt wie überall. */
export function MapPhotosDialog({ mission, onClose }: { mission: Mission; onClose: () => void }) {
  return (
    <Modal open title={`Kartenfotos · ${mission.name}`} footer={null} onCancel={onClose}>
      <p className="sl-mission-fm-sections">Ein Klick öffnet das Foto in voller Größe in einem neuen Tab.</p>
      <ul className="sl-map-photos">
        {mission.sections
          .filter((s) => s.has_map_photo)
          .map((s) => (
            <li key={s.id}>
              <span>Abschnitt {s.label}</span>
              <MapPhotoThumb missionId={mission.id} section={s} />
            </li>
          ))}
      </ul>
    </Modal>
  );
}

export function EditDialog({ mission, onClose }: { mission: Mission; onClose: () => void }) {
  const update = useUpdateMission();
  const [form] = Form.useForm<{ name: string; location?: string | null }>();
  return (
    <Modal
      open
      title="Einsatz bearbeiten"
      okText="Speichern"
      cancelText="Abbrechen"
      confirmLoading={update.isPending}
      onOk={() => form.submit()}
      onCancel={onClose}
    >
      <Form
        form={form}
        layout="vertical"
        requiredMark={false}
        initialValues={{ name: mission.name, location: mission.location }}
        onFinish={(v) =>
          void update
            .mutateAsync({ id: mission.id, name: v.name, location: v.location || null })
            .then(onClose, () => {})
        }
      >
        <Form.Item label="Name" name="name" rules={[{ required: true, message: 'Name fehlt' }]}>
          <Input />
        </Form.Item>
        <Form.Item label="Ort" name="location">
          <Input />
        </Form.Item>
      </Form>
      {update.isError && <Alert type="error" title={errorText(update.error)} />}
    </Modal>
  );
}

/** `onDone`: wohin nach dem Abschluss; ohne geht es zur Startseite. */
export function CloseDialog({
  mission,
  onClose,
  onDone,
}: {
  mission: Mission;
  onClose: () => void;
  onDone?: () => void;
}) {
  const close = useCloseMission();
  const { reset } = useSkip();
  const navigate = useNavigate();
  return (
    <Modal
      open
      title={`Einsatz „${mission.name}“ abschließen?`}
      okText="Abschließen"
      cancelText="Abbrechen"
      okButtonProps={{ danger: true }}
      confirmLoading={close.isPending}
      // Danach ist die Kopfleiste schon „Archiv“ und dieser Dialog weg: darum das Promise. Ein früheres
      // „Überspringen“ gilt nicht mehr, nach dem Abschluss kommt die Startseite.
      onOk={() =>
        void close.mutateAsync(mission.id).then(() => {
          reset();
          if (onDone) onDone();
          else void navigate('/');
        }, () => {})
      }
      onCancel={onClose}
    >
      <p>Abgeschlossene Einsätze sind nur noch lesbar.</p>
      {close.isError && <Alert type="error" title={errorText(close.error)} />}
    </Modal>
  );
}
