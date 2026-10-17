import { Alert, Button, Form, Input, InputNumber, Switch, type FormInstance } from 'antd';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { useState } from 'react';
import { ApiError, apiSend } from '../api/client';
import { queryKeys } from '../api/queries';
import type { BrokerSettingsInput } from '../contract/generated/BrokerSettingsInput';
import type { BrokerSettingsView } from '../contract/generated/BrokerSettingsView';
import type { DjiSettingsInput } from '../contract/generated/DjiSettingsInput';
import type { DjiSettingsView } from '../contract/generated/DjiSettingsView';
import type { GeneralSettings } from '../contract/generated/GeneralSettings';
import type { MediaSettings } from '../contract/generated/MediaSettings';
import type { OidcSettingsInput } from '../contract/generated/OidcSettingsInput';
import type { OidcSettingsView } from '../contract/generated/OidcSettingsView';
import type { SettingsView } from '../contract/generated/SettingsView';

const KEEP = 'gespeichert – leer lassen zum Behalten';

/** Ein leeres Geheimnisfeld behält den gespeicherten Wert: Der Server bekommt `null`. */
export function secretOrNull(v: string | undefined): string | null {
  return v && v.trim() ? v.trim() : null;
}

/** Im Formular ist ein Geheimnis ein optionaler Text, im Vertrag `string | null`. */
type WithSecrets<I, K extends keyof I> = Omit<I, K> & { [P in K]?: string };

/**
 * Speichert einen Abschnitt und legt die Antwort des Servers als neuen Stand ab. Danach sind die
 * Geheimnisfelder wieder leer, damit der Platzhalter „gespeichert“ stimmt.
 */
function useSave<I>(section: string, form: FormInstance, secretFields: string[] = []) {
  const client = useQueryClient();
  const [done, setDone] = useState(false);
  const m = useMutation({
    mutationFn: (body: I) => apiSend<SettingsView>('PUT', `/api/settings/${section}`, body),
    onMutate: () => setDone(false),
    onSuccess: (view) => {
      client.setQueryData(queryKeys.settings, view);
      if (secretFields.length) form.resetFields(secretFields);
      setDone(true);
    },
  });
  const error =
    m.error instanceof ApiError
      ? m.error.message
      : m.error
        ? 'Nicht gespeichert – Server nicht erreichbar.'
        : null;
  return { save: m.mutate, pending: m.isPending, done, error };
}

function Result({ done, error }: { done: boolean; error: string | null }) {
  if (error) return <Alert type="error" title={error} className="sl-form-result" />;
  return done ? (
    <p role="status" className="sl-ok">
      Gespeichert.
    </p>
  ) : null;
}

type OidcValues = WithSecrets<OidcSettingsInput, 'client_secret'>;

/** `redirectUrl` kommt vom Server (`AuthOptions`), solange noch keine Anmeldung gespeichert ist. */
export function OidcForm({ view, redirectUrl }: { view: OidcSettingsView | null; redirectUrl?: string }) {
  const [form] = Form.useForm<OidcValues>();
  const s = useSave<OidcSettingsInput>('oidc', form, ['client_secret']);
  const callback = view?.redirect_url ?? redirectUrl;
  return (
    <Form<OidcValues>
      form={form}
      layout="vertical"
      className="sl-form"
      initialValues={{ issuer: view?.issuer, client_id: view?.client_id }}
      onFinish={(v) =>
        s.save({ issuer: v.issuer, client_id: v.client_id, client_secret: secretOrNull(v.client_secret) })
      }
    >
      {callback && (
        <p className="sl-form-hint">
          In Pocket ID als Callback-URL eintragen: <code className="sl-code">{callback}</code>
        </p>
      )}
      <Form.Item label="Issuer" name="issuer" rules={[{ required: true, message: 'Issuer fehlt' }]}>
        <Input placeholder="https://id.example.org" />
      </Form.Item>
      <Form.Item label="Client-ID" name="client_id" rules={[{ required: true, message: 'Client-ID fehlt' }]}>
        <Input />
      </Form.Item>
      <Form.Item label="Client-Secret" name="client_secret">
        <Input.Password
          placeholder={view?.has_client_secret ? KEEP : undefined}
          autoComplete="new-password"
        />
      </Form.Item>
      <Result done={s.done} error={s.error} />
      <Button type="primary" htmlType="submit" loading={s.pending}>
        Anmeldung speichern
      </Button>
    </Form>
  );
}

type DjiValues = WithSecrets<DjiSettingsInput, 'app_key' | 'license'>;

export function DjiForm({ view }: { view: DjiSettingsView | null }) {
  const [form] = Form.useForm<DjiValues>();
  const s = useSave<DjiSettingsInput>('dji', form, ['app_key', 'license']);
  return (
    <Form<DjiValues>
      form={form}
      layout="vertical"
      className="sl-form"
      initialValues={{ app_id: view?.app_id, workspace_id: view?.workspace_id }}
      onFinish={(v) =>
        s.save({
          app_id: v.app_id,
          app_key: secretOrNull(v.app_key),
          license: secretOrNull(v.license),
          workspace_id: v.workspace_id,
        })
      }
    >
      <Form.Item label="App-ID" name="app_id" rules={[{ required: true, message: 'App-ID fehlt' }]}>
        <Input />
      </Form.Item>
      <Form.Item label="App-Key" name="app_key">
        <Input.Password placeholder={view?.has_app_key ? KEEP : undefined} autoComplete="new-password" />
      </Form.Item>
      <Form.Item label="Lizenz" name="license">
        <Input.Password placeholder={view?.has_license ? KEEP : undefined} autoComplete="new-password" />
      </Form.Item>
      <Form.Item
        label="Workspace-ID"
        name="workspace_id"
        rules={[{ required: true, message: 'Workspace-ID fehlt' }]}
      >
        <Input />
      </Form.Item>
      <Result done={s.done} error={s.error} />
      <Button type="primary" htmlType="submit" loading={s.pending}>
        DJI-Daten speichern
      </Button>
    </Form>
  );
}

type BrokerValues = WithSecrets<BrokerSettingsInput, 'admin_password'>;

export function BrokerForm({ view }: { view: BrokerSettingsView | null }) {
  const [form] = Form.useForm<BrokerValues>();
  const s = useSave<BrokerSettingsInput>('broker', form, ['admin_password']);
  return (
    <Form<BrokerValues>
      form={form}
      layout="vertical"
      className="sl-form"
      initialValues={{
        host: view?.host,
        port: view?.port ?? 1883,
        admin_username: view?.admin_username,
        rc_url: view?.rc_url,
      }}
      onFinish={(v) =>
        s.save({
          host: v.host,
          port: v.port,
          admin_username: v.admin_username,
          admin_password: secretOrNull(v.admin_password),
          rc_url: v.rc_url,
        })
      }
    >
      <Form.Item
        label="Broker-Adresse (vom Server aus)"
        name="host"
        rules={[{ required: true, message: 'Adresse fehlt' }]}
      >
        <Input placeholder="mqtt" />
      </Form.Item>
      <Form.Item label="Port" name="port" rules={[{ required: true, message: 'Port fehlt' }]}>
        <InputNumber min={1} max={65535} precision={0} />
      </Form.Item>
      <Form.Item
        label="Admin-Benutzer"
        name="admin_username"
        rules={[{ required: true, message: 'Benutzer fehlt' }]}
      >
        <Input />
      </Form.Item>
      <Form.Item label="Admin-Passwort" name="admin_password">
        <Input.Password
          placeholder={view?.has_admin_password ? KEEP : undefined}
          autoComplete="new-password"
        />
      </Form.Item>
      <Form.Item
        label="Adresse für die Fernsteuerung (im VPN)"
        name="rc_url"
        rules={[{ required: true, message: 'Adresse fehlt' }]}
      >
        <Input placeholder="tcp://100.64.0.10:1883" />
      </Form.Item>
      <Result done={s.done} error={s.error} />
      <Button type="primary" htmlType="submit" loading={s.pending}>
        Broker speichern
      </Button>
    </Form>
  );
}

/** Vorgaben für die erste Einrichtung: die Dienstnamen aus `deploy/compose.m1.yaml`. */
const MEDIA_DEFAULTS: MediaSettings = {
  webrtc_url: 'http://media:8889',
  hls_url: 'http://media:8888',
  playback_url: 'http://media:9996',
  rtmp_url: '',
};

export function MediaForm({ view }: { view: MediaSettings | null }) {
  const [form] = Form.useForm<MediaSettings>();
  const s = useSave<MediaSettings>('media', form);
  const required = (what: string) => [{ required: true, message: `${what} fehlt` }];
  return (
    <Form<MediaSettings>
      form={form}
      layout="vertical"
      className="sl-form"
      initialValues={view ?? MEDIA_DEFAULTS}
      onFinish={(v) => s.save(v)}
    >
      <Form.Item label="WebRTC (intern)" name="webrtc_url" rules={required('WebRTC-Adresse')}>
        <Input />
      </Form.Item>
      <Form.Item label="HLS (intern)" name="hls_url" rules={required('HLS-Adresse')}>
        <Input />
      </Form.Item>
      <Form.Item label="Playback (intern)" name="playback_url" rules={required('Playback-Adresse')}>
        <Input />
      </Form.Item>
      <Form.Item label="RTMP für die Fernsteuerung (im VPN)" name="rtmp_url" rules={required('RTMP-Adresse')}>
        <Input placeholder="rtmp://100.64.0.10:1935" />
      </Form.Item>
      <Result done={s.done} error={s.error} />
      <Button type="primary" htmlType="submit" loading={s.pending}>
        Video speichern
      </Button>
    </Form>
  );
}

export function GeneralForm({ view }: { view: GeneralSettings }) {
  const [form] = Form.useForm<GeneralSettings>();
  const s = useSave<GeneralSettings>('general', form);
  return (
    <Form<GeneralSettings>
      form={form}
      layout="vertical"
      className="sl-form"
      initialValues={view}
      onFinish={(v) => s.save(v)}
    >
      <Form.Item
        label="Livebild beim Abheben automatisch starten"
        name="auto_live_on_takeoff"
        valuePropName="checked"
      >
        <Switch />
      </Form.Item>
      <Form.Item
        label="Rohe MQTT-Nachrichten mitschneiden (Testdaten aus echten Flügen)"
        name="record_raw_mqtt"
        valuePropName="checked"
      >
        <Switch />
      </Form.Item>
      <Result done={s.done} error={s.error} />
      <Button type="primary" htmlType="submit" loading={s.pending}>
        Allgemeines speichern
      </Button>
    </Form>
  );
}
