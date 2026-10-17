import { Alert, Button, Form, Input, Popconfirm } from 'antd';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { useState } from 'react';
import { ApiError, apiSend } from '../api/client';
import { queryKeys, useKioskTokens } from '../api/queries';
import { PaneelZustand } from '../components/instrument';
import type { KioskTokenCreate } from '../contract/generated/KioskTokenCreate';
import type { KioskTokenCreated } from '../contract/generated/KioskTokenCreated';
import { OnceVisibleLink } from './OnceVisibleLink';
import { zustandOf } from './zustand';

const createdAt = (ms: number) =>
  new Date(ms).toLocaleString('de-DE', {
    day: '2-digit',
    month: '2-digit',
    year: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  });

export function KioskTokensPanel() {
  const query = useKioskTokens();
  const { data } = query;
  const client = useQueryClient();
  const [form] = Form.useForm<KioskTokenCreate>();
  const [created, setCreated] = useState<KioskTokenCreated | null>(null);
  const refresh = () => client.invalidateQueries({ queryKey: queryKeys.kioskTokens });
  const create = useMutation({
    mutationFn: (body: KioskTokenCreate) => apiSend<KioskTokenCreated>('POST', '/api/kiosk-tokens', body),
    onSuccess: (c) => {
      setCreated(c);
      form.resetFields();
      void refresh();
    },
  });
  const remove = useMutation({
    mutationFn: (id: number) => apiSend<void>('DELETE', `/api/kiosk-tokens/${id}`),
    onSuccess: () => void refresh(),
  });
  const error = create.error ?? remove.error;
  return (
    <div className="sl-stack">
      <PaneelZustand
        zustand={zustandOf(query)}
        titel="Großbild-Zugänge"
        leerText="Noch kein Großbild gekoppelt. Unten einen Zugang erzeugen und den Link am Großbild öffnen."
        onNeuladen={() => void query.refetch()}
      >
        <div className="sl-table-scroll">
          <table className="sl-table">
            <thead>
              <tr>
                <th scope="col">Bezeichnung</th>
                <th scope="col">Erstellt</th>
                <th scope="col">
                  <span className="sl-sr">Aktion</span>
                </th>
              </tr>
            </thead>
            <tbody>
              {data?.map((t) => (
                <tr key={t.id}>
                  <td>{t.label}</td>
                  <td className="sl-num">{createdAt(t.created_at_ms)}</td>
                  <td className="sl-cell-action">
                    <Popconfirm
                      title={`Großbild „${t.label}“ abmelden?`}
                      description="Der Bildschirm verliert den Zugang und muss neu gekoppelt werden."
                      okText="Widerrufen"
                      okButtonProps={{ danger: true }}
                      cancelText="Abbrechen"
                      onConfirm={() => remove.mutate(t.id)}
                    >
                      <Button danger size="small" aria-label={`Zugang widerrufen: ${t.label}`}>
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
      <Form<KioskTokenCreate>
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
          <Input placeholder="FüKw Großbild" />
        </Form.Item>
        <Button type="primary" htmlType="submit" loading={create.isPending}>
          Großbild-Zugang erzeugen
        </Button>
      </Form>
      {created && (
        <OnceVisibleLink
          url={created.url}
          hint="Auf dem Großbild-Rechner im Browser öffnen; er bleibt angemeldet, bis der Zugang widerrufen wird."
        />
      )}
    </div>
  );
}
