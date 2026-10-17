import { Alert, Button, Form, Input, Popconfirm } from 'antd';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { useState } from 'react';
import { ApiError, apiSend } from '../api/client';
import { queryKeys, useRemoteControllers } from '../api/queries';
import { PaneelZustand } from '../components/instrument';
import type { RemoteControllerCreate } from '../contract/generated/RemoteControllerCreate';
import type { RemoteControllerCreated } from '../contract/generated/RemoteControllerCreated';
import { OnceVisibleLink } from './OnceVisibleLink';
import { zustandOf } from './zustand';

const pairedAt = (ms: number) =>
  new Date(ms).toLocaleString('de-DE', {
    day: '2-digit',
    month: '2-digit',
    year: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  });

export function RemoteControllersPanel() {
  const query = useRemoteControllers();
  const { data } = query;
  const client = useQueryClient();
  const [form] = Form.useForm<RemoteControllerCreate>();
  const [created, setCreated] = useState<RemoteControllerCreated | null>(null);
  const refresh = () => client.invalidateQueries({ queryKey: queryKeys.remoteControllers });
  const create = useMutation({
    mutationFn: (body: RemoteControllerCreate) =>
      apiSend<RemoteControllerCreated>('POST', '/api/remote-controllers', body),
    onSuccess: (c) => {
      setCreated(c);
      form.resetFields();
      void refresh();
    },
  });
  const remove = useMutation({
    mutationFn: (id: number) => apiSend<void>('DELETE', `/api/remote-controllers/${id}`),
    onSuccess: () => void refresh(),
  });
  const error = create.error ?? remove.error;
  return (
    <div className="sl-stack">
      <PaneelZustand
        zustand={zustandOf(query)}
        titel="Fernsteuerungen"
        leerText="Noch keine Fernsteuerung. Unten eine anlegen und die Adresse in Pilot 2 öffnen."
        onNeuladen={() => void query.refetch()}
      >
        <div className="sl-table-scroll">
          <table className="sl-table">
            <thead>
              <tr>
                <th scope="col">Bezeichnung</th>
                <th scope="col">Seriennummer</th>
                <th scope="col">Gekoppelt</th>
                <th scope="col">
                  <span className="sl-sr">Aktion</span>
                </th>
              </tr>
            </thead>
            <tbody>
              {data?.map((rc) => (
                <tr key={rc.id}>
                  <td>{rc.label}</td>
                  <td>
                    {rc.sn != null ? (
                      <span className="sl-num">{rc.sn}</span>
                    ) : (
                      <span className="sl-muted">noch nicht gekoppelt</span>
                    )}
                  </td>
                  <td className="sl-num">{rc.paired_at_ms != null ? pairedAt(rc.paired_at_ms) : '–'}</td>
                  <td className="sl-cell-action">
                    <Popconfirm
                      title={`Zugang von ${rc.label} widerrufen?`}
                      description="Die Fernsteuerung verliert sofort die Verbindung zum Broker."
                      okText="Widerrufen"
                      okButtonProps={{ danger: true }}
                      cancelText="Abbrechen"
                      onConfirm={() => remove.mutate(rc.id)}
                    >
                      <Button danger size="small" aria-label={`Zugang widerrufen: ${rc.label}`}>
                        Zugang widerrufen
                      </Button>
                    </Popconfirm>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </PaneelZustand>
      {error && (
        <Alert type="error" title={error instanceof ApiError ? error.message : 'Server nicht erreichbar.'} />
      )}
      <Form<RemoteControllerCreate>
        form={form}
        layout="inline"
        className="sl-create"
        colon={false}
        requiredMark={false}
        onFinish={(v) => create.mutate(v)}
      >
        <Form.Item
          label="Bezeichnung"
          name="label"
          rules={[{ required: true, message: 'Bezeichnung fehlt' }]}
        >
          <Input placeholder="RC Plus 1" />
        </Form.Item>
        <Button type="primary" htmlType="submit" loading={create.isPending}>
          Fernsteuerung hinzufügen
        </Button>
      </Form>
      {created && (
        <OnceVisibleLink
          url={created.pairing_url}
          hint={`In Pilot 2 auf „${created.controller.label}“ unter Cloud-Dienst → Open Platform eintragen.`}
        />
      )}
    </div>
  );
}
