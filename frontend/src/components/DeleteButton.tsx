import { Alert, Button, Modal } from 'antd';
import { useState, type ReactNode } from 'react';
import { ApiError } from '../api/client';

interface Props {
  /** Knopftext, etwa „Flug löschen …“. */
  label: string;
  /** Frage im Dialog, etwa „Flug 6 löschen?“. */
  title: string;
  /** Was mitgeht. */
  children: ReactNode;
  pending: boolean;
  error: unknown;
  /** `reset` der Mutation: Ein alter Fehler steht beim nächsten Öffnen nicht mehr im Dialog. */
  onOpen: () => void;
  onConfirm: () => Promise<unknown>;
}

/**
 * Löschen mit Rückfrage (nur Verwaltung). Ein Dialog statt Popconfirm: Der Text des Servers (422, 503)
 * bleibt neben dem Knopf stehen, bis man es erneut versucht oder den Dialog neu öffnet.
 */
export function DeleteButton({ label, title, children, pending, error, onOpen, onConfirm }: Props) {
  const [open, setOpen] = useState(false);
  return (
    <>
      <Button
        size="small"
        danger
        onClick={() => {
          onOpen();
          setOpen(true);
        }}
      >
        {label}
      </Button>
      <Modal
        open={open}
        title={title}
        okText="Endgültig löschen"
        cancelText="Abbrechen"
        okButtonProps={{ danger: true }}
        confirmLoading={pending}
        onOk={() => void onConfirm().then(() => setOpen(false), () => {})}
        onCancel={() => setOpen(false)}
      >
        <p>{children} Das lässt sich nicht rückgängig machen.</p>
        {error != null && (
          <Alert type="error" title={error instanceof ApiError ? error.message : 'Server nicht erreichbar.'} />
        )}
      </Modal>
    </>
  );
}
