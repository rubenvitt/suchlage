import { Alert, Button, Form, Input, Modal } from 'antd';
import { useRef, useState } from 'react';
import { useNavigate } from 'react-router';
import { ApiError } from '../api/client';
import { useCreateMission, useStartMission } from '../api/queries';

const errorText = (e: unknown) => (e instanceof ApiError ? e.message : 'Server nicht erreichbar.');

/**
 * Neuer Einsatz aus der Einsatzliste. Läuft schon einer, gibt es nur „Vorbereiten“ (der Server lehnte
 * den Start mit 409 ab). Vorbereitet geht es auf die Einsatzseite, gestartet auf die Lage.
 */
export function NewMissionDialog({ running, onClose }: { running: boolean; onClose: () => void }) {
  const [form] = Form.useForm<{ name: string; location?: string }>();
  const navigate = useNavigate();
  const create = useCreateMission();
  const start = useStartMission();
  // Angelegt, aber der Start scheiterte: kein zweites Anlegen beim nächsten Klick.
  const [createdId, setCreatedId] = useState<number | null>(null);
  const busy = create.isPending || start.isPending;
  const error = create.error ?? start.error;

  // Synchrone Sperre: zweimal Enter schneller als der nächste Render legt sonst zwei Einsätze an.
  const sending = useRef(false);

  const send = async (run: boolean) => {
    if (sending.current) return;
    sending.current = true;
    try {
      await submit(run);
    } finally {
      sending.current = false;
    }
  };

  const submit = async (run: boolean) => {
    const v = await form.validateFields().catch(() => null);
    if (!v) return;
    const id =
      createdId ??
      (await create
        .mutateAsync({ name: v.name, location: v.location?.trim() || null })
        .then((m) => m.id)
        .catch(() => null));
    if (id == null) return;
    setCreatedId(id);
    if (!run) return void navigate(`/einsaetze/${id}`);
    await start.mutateAsync(id).then(
      () => navigate('/'),
      () => {},
    );
  };

  return (
    <Modal
      open
      title="Neuer Einsatz"
      onCancel={onClose}
      footer={
        <div className="sl-dialog-actions">
          <Button onClick={onClose}>Abbrechen</Button>
          <Button type={running ? 'primary' : 'default'} loading={create.isPending} disabled={start.isPending} onClick={() => void send(false)}>
            Vorbereiten
          </Button>
          {!running && (
            <Button type="primary" loading={start.isPending} disabled={busy && !start.isPending} onClick={() => void send(true)}>
              Anlegen und starten
            </Button>
          )}
        </div>
      }
    >
      <Form form={form} layout="vertical" requiredMark={false} onFinish={() => void send(!running)}>
        <Form.Item label="Einsatzname" name="name" rules={[{ required: true, whitespace: true, message: 'Name fehlt' }]}>
          <Input autoFocus placeholder="Vermisste Person Ebstorf" disabled={createdId != null} />
        </Form.Item>
        <Form.Item label="Ort" name="location">
          <Input placeholder="Waldgebiet Hanstedt" disabled={createdId != null} />
        </Form.Item>
        {/* Enter im Feld sendet. */}
        <button type="submit" hidden />
      </Form>
      {error && (
        <Alert
          type="error"
          title={
            error instanceof ApiError && error.status === 409 ? 'Es läuft schon ein Einsatz' : errorText(error)
          }
        />
      )}
    </Modal>
  );
}
