import { Alert, Button, Form, Input } from 'antd';
import { useState } from 'react';
import { ApiError, apiSend } from '../api/client';
import { useAuthOptions } from '../api/queries';
import type { SetupRequest } from '../contract/generated/SetupRequest';
import type { SetupResult } from '../contract/generated/SetupResult';
import './admin.css';

type SetupValues = Omit<SetupRequest, 'token'>;

/** Öffentlich. Der Einrichtungslink aus dem Server-Protokoll trägt das Token hinter `#`. */
export function SetupPage() {
  const [token] = useState(() => window.location.hash.slice(1));
  const [error, setError] = useState<string | null>(null);
  const [pending, setPending] = useState(false);
  // Vom Server (SUCHLAGE_PUBLIC_URL), nicht aus der Adresszeile: hinter Proxy oder Vite wiche die ab.
  const options = useAuthOptions();
  const callback = options.data?.redirect_url;

  async function submit(v: SetupValues) {
    setPending(true);
    setError(null);
    try {
      const res = await apiSend<SetupResult>('POST', '/api/setup', { token, ...v } satisfies SetupRequest);
      window.location.assign(res.login_url);
    } catch (e) {
      setError(e instanceof ApiError ? e.message : 'Server nicht erreichbar.');
    } finally {
      setPending(false);
    }
  }

  return (
    <div className="sl-center">
      <section className="sl-card" aria-labelledby="setup-title">
        <h1 id="setup-title">Suchlage einrichten</h1>
        {!token && (
          <Alert
            type="error"
            title="Der Einrichtungslink ist unvollständig. Den Link aus dem Server-Protokoll vollständig öffnen."
          />
        )}
        <p>In Pocket ID einen OIDC-Client anlegen und diese Callback-URL eintragen:</p>
        {callback ? (
          <code className="sl-once-url">{callback}</code>
        ) : (
          <p aria-busy={options.isPending}>
            {options.isError
              ? 'Server nicht erreichbar – die Callback-URL lässt sich gerade nicht abrufen.'
              : 'Callback-URL wird abgerufen …'}
          </p>
        )}
        <Form<SetupValues> layout="vertical" onFinish={submit} requiredMark={false}>
          <Form.Item label="Issuer" name="issuer" rules={[{ required: true, message: 'Issuer fehlt' }]}>
            <Input placeholder="https://id.example.org" />
          </Form.Item>
          <Form.Item
            label="Client-ID"
            name="client_id"
            rules={[{ required: true, message: 'Client-ID fehlt' }]}
          >
            <Input />
          </Form.Item>
          <Form.Item
            label="Client-Secret"
            name="client_secret"
            rules={[{ required: true, message: 'Client-Secret fehlt' }]}
          >
            <Input.Password autoComplete="new-password" />
          </Form.Item>
          {error && <Alert type="error" title={error} className="sl-form-result" />}
          <Button type="primary" htmlType="submit" loading={pending} disabled={!token} block>
            Einrichten und anmelden
          </Button>
        </Form>
        <p className="sl-muted">Wer sich als Erstes anmeldet, verwaltet Suchlage.</p>
      </section>
    </div>
  );
}
