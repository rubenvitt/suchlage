import { Alert, Button, Popconfirm } from 'antd';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { ApiError, apiSend } from '../api/client';
import { queryKeys, useAccounts } from '../api/queries';
import { PaneelZustand } from '../components/instrument';
import type { AccountView } from '../contract/generated/AccountView';
import { zustandOf } from './zustand';

const at = (ms: number) =>
  new Date(ms).toLocaleString('de-DE', {
    day: '2-digit',
    month: '2-digit',
    year: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  });

const ROLE: Record<AccountView['role'], string> = {
  admin: 'Verwaltung',
  operator: 'Nutzung',
  kiosk: 'Großbild',
};

/** Konten aus Pocket ID. Die Rolle folgt den Gruppen; hier lässt sich ein Konto nur sperren. */
export function AccountsPanel() {
  const query = useAccounts();
  const { data } = query;
  const client = useQueryClient();
  const refresh = () => client.invalidateQueries({ queryKey: queryKeys.accounts });
  const block = useMutation({
    mutationFn: (id: number) => apiSend<void>('PUT', `/api/accounts/${id}/block`),
    onSuccess: () => void refresh(),
  });
  const unblock = useMutation({
    mutationFn: (id: number) => apiSend<void>('DELETE', `/api/accounts/${id}/block`),
    onSuccess: () => void refresh(),
  });
  const error = block.error ?? unblock.error;
  return (
    <div className="sl-stack">
      <PaneelZustand
        zustand={zustandOf(query)}
        titel="Konten"
        leerText="Noch hat sich niemand angemeldet."
        onNeuladen={() => void query.refetch()}
      >
        <div className="sl-table-scroll">
          <table className="sl-table">
            <thead>
              <tr>
                <th scope="col">Name</th>
                <th scope="col">Rolle</th>
                <th scope="col">Zuerst angemeldet</th>
                <th scope="col">Zugang</th>
                <th scope="col">
                  <span className="sl-sr">Aktion</span>
                </th>
              </tr>
            </thead>
            <tbody>
              {data?.map((a) => (
                <tr key={a.id}>
                  <td>
                    {a.display_name}
                    {a.own && <span className="sl-muted"> (eigenes Konto)</span>}
                  </td>
                  <td>{ROLE[a.role]}</td>
                  <td className="sl-num">{at(a.created_at_ms)}</td>
                  <td>{a.blocked_at_ms === null ? 'frei' : `gesperrt seit ${at(a.blocked_at_ms)}`}</td>
                  <td className="sl-cell-action">
                    {a.own ? null : a.blocked_at_ms === null ? (
                      <Popconfirm
                        title={`Konto „${a.display_name}“ sperren?`}
                        description="Alle Sitzungen enden sofort. Eine neue Anmeldung wird abgelehnt, auch wenn das Konto in Pocket ID noch in einer Suchlage-Gruppe ist."
                        okText="Sperren"
                        okButtonProps={{ danger: true }}
                        cancelText="Abbrechen"
                        onConfirm={() => block.mutate(a.id)}
                      >
                        <Button danger size="small" aria-label={`Konto sperren: ${a.display_name}`}>
                          Sperren
                        </Button>
                      </Popconfirm>
                    ) : (
                      <Button
                        size="small"
                        aria-label={`Sperre aufheben: ${a.display_name}`}
                        onClick={() => unblock.mutate(a.id)}
                      >
                        Sperre aufheben
                      </Button>
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </PaneelZustand>
      <p className="sl-form-hint">
        Die Rolle kommt bei jeder Anmeldung aus den Pocket-ID-Gruppen. Wer dauerhaft keinen Zugang
        mehr haben soll, gehört dort aus den Gruppen entfernt; die Sperre hier wirkt sofort.
      </p>
      {error && (
        <Alert type="error" title={error instanceof ApiError ? error.message : 'Server nicht erreichbar.'} />
      )}
    </div>
  );
}
