import { Alert, Button, Form, Input, InputNumber, Space, Switch, type FormInstance } from 'antd';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { useState } from 'react';
import { ApiError, apiSend } from '../api/client';
import { queryKeys } from '../api/queries';
import { StatusChip } from '../components/instrument';
import type { BrokerSettingsInput } from '../contract/generated/BrokerSettingsInput';
import type { BrokerSettingsView } from '../contract/generated/BrokerSettingsView';
import type { DjiSettingsInput } from '../contract/generated/DjiSettingsInput';
import type { DjiSettingsView } from '../contract/generated/DjiSettingsView';
import type { FlugmappeSettings } from '../contract/generated/FlugmappeSettings';
import type { GeneralSettings } from '../contract/generated/GeneralSettings';
import type { MediaSettings } from '../contract/generated/MediaSettings';
import type { OidcSettingsInput } from '../contract/generated/OidcSettingsInput';
import type { OidcSettingsView } from '../contract/generated/OidcSettingsView';
import type { PhotoStorageSettingsInput } from '../contract/generated/PhotoStorageSettingsInput';
import type { PhotoStorageSettingsView } from '../contract/generated/PhotoStorageSettingsView';
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
  return { save: m.mutate, pending: m.isPending, done, error: saveError(m.error) };
}

/** Fehlertext am Formular: der Satz des Servers, sonst „nicht erreichbar“. */
function saveError(e: unknown): string | null {
  return e instanceof ApiError ? e.message : e ? 'Nicht gespeichert – Server nicht erreichbar.' : null;
}

function Result({
  done,
  error,
  text = 'Gespeichert.',
}: {
  done: boolean;
  error: string | null;
  text?: string;
}) {
  if (error) return <Alert type="error" title={error} className="sl-form-result" />;
  return done ? (
    <p role="status" className="sl-ok">
      {text}
    </p>
  ) : null;
}

type OidcValues = WithSecrets<OidcSettingsInput, 'client_secret'>;

/** Gruppen aus Pocket ID; auch in der Ersteinrichtung. Ohne eine der beiden kein Zugang. */
export function OidcGroupFields() {
  return (
    <>
      <p className="sl-form-hint">
        Gruppennamen wie in Pocket ID (Feld „Name“, nicht der Anzeigename). Wer in keiner der beiden
        Gruppen ist, kann sich nicht anmelden. Die Rolle gilt ab der nächsten Anmeldung.
      </p>
      <Form.Item
        label="Gruppe Verwaltung (Admin)"
        name="admin_group"
        rules={[{ required: true, whitespace: true, message: 'Gruppe fehlt' }]}
      >
        <Input placeholder="suchlage-admin" />
      </Form.Item>
      <Form.Item
        label="Gruppe Nutzung (Operator)"
        name="user_group"
        rules={[{ required: true, whitespace: true, message: 'Gruppe fehlt' }]}
      >
        <Input placeholder="suchlage" />
      </Form.Item>
    </>
  );
}

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
      initialValues={{
        issuer: view?.issuer,
        client_id: view?.client_id,
        admin_group: view?.admin_group,
        user_group: view?.user_group,
      }}
      onFinish={(v) =>
        s.save({
          issuer: v.issuer,
          client_id: v.client_id,
          client_secret: secretOrNull(v.client_secret),
          admin_group: v.admin_group,
          user_group: v.user_group,
        })
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
      <OidcGroupFields />
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

type PhotoStorageValues = WithSecrets<PhotoStorageSettingsInput, 'secret_access_key' | 'admin_token'>;

/** Vorgaben für die erste Einrichtung: Dienstname und Bucket aus `deploy/fotos-init.sh`. */
const PHOTO_STORAGE_DEFAULTS: Partial<PhotoStorageValues> = {
  s3_url: 'http://fotos:3900',
  admin_url: 'http://fotos:3903',
  bucket: 'suchlage-fotos',
  region: 'garage',
};

export function PhotoStorageForm({ view }: { view: PhotoStorageSettingsView | null }) {
  const [form] = Form.useForm<PhotoStorageValues>();
  const s = useSave<PhotoStorageSettingsInput>('photo_storage', form, ['secret_access_key', 'admin_token']);
  const required = (what: string) => [{ required: true, message: `${what} fehlt` }];
  return (
    <Form<PhotoStorageValues>
      form={form}
      layout="vertical"
      className="sl-form"
      initialValues={
        view
          ? {
              s3_url: view.s3_url,
              rc_url: view.rc_url,
              admin_url: view.admin_url,
              bucket: view.bucket,
              region: view.region,
              access_key_id: view.access_key_id,
            }
          : PHOTO_STORAGE_DEFAULTS
      }
      onFinish={(v) =>
        s.save({
          s3_url: v.s3_url,
          rc_url: v.rc_url,
          admin_url: v.admin_url,
          bucket: v.bucket,
          region: v.region,
          access_key_id: v.access_key_id,
          secret_access_key: secretOrNull(v.secret_access_key),
          admin_token: secretOrNull(v.admin_token),
        })
      }
    >
      <p className="sl-form-hint">
        Die Werte gibt <code className="sl-code">deploy/fotos-init.sh</code> einmal aus.
      </p>
      <Form.Item label="S3-Adresse (vom Server aus)" name="s3_url" rules={required('S3-Adresse')}>
        <Input placeholder="http://fotos:3900" />
      </Form.Item>
      <Form.Item label="S3-Adresse für die Fernsteuerung (im VPN)" name="rc_url" rules={required('Adresse')}>
        <Input placeholder="http://100.64.0.10:3900" />
      </Form.Item>
      <Form.Item label="Admin-Adresse (vom Server aus)" name="admin_url" rules={required('Admin-Adresse')}>
        <Input placeholder="http://fotos:3903" />
      </Form.Item>
      <Form.Item label="Bucket" name="bucket" rules={required('Bucket')}>
        <Input />
      </Form.Item>
      <Form.Item label="Region" name="region" rules={required('Region')}>
        <Input />
      </Form.Item>
      <Form.Item label="Schlüssel-ID des Servers" name="access_key_id" rules={required('Schlüssel-ID')}>
        <Input placeholder="GK…" />
      </Form.Item>
      <Form.Item label="Geheimer Schlüssel des Servers" name="secret_access_key">
        <Input.Password
          placeholder={view?.has_secret_access_key ? KEEP : undefined}
          autoComplete="new-password"
        />
      </Form.Item>
      <Form.Item label="Admin-Token" name="admin_token">
        <Input.Password placeholder={view?.has_admin_token ? KEEP : undefined} autoComplete="new-password" />
      </Form.Item>
      <Result done={s.done} error={s.error} />
      <Button type="primary" htmlType="submit" loading={s.pending}>
        Fotospeicher speichern
      </Button>
    </Form>
  );
}

const EMPTY_FLUGMAPPE: FlugmappeSettings = { client_id: '', allowed_origin: '' };

/**
 * Schnittstelle für die Flugmappe (SUL-66). „Ausschalten“ entfernt die Einstellung, die Felder
 * bleiben stehen: „Speichern“ schaltet sie mit denselben Werten wieder ein. Ein leeres Formular
 * schaltet ebenfalls aus. Der Zustand folgt der letzten Antwort des Servers, sonst `view`.
 */
export function FlugmappeForm({ view }: { view: FlugmappeSettings | null }) {
  const [form] = Form.useForm<FlugmappeSettings>();
  const client = useQueryClient();
  const m = useMutation({
    mutationFn: (body: FlugmappeSettings | null) =>
      body
        ? apiSend<SettingsView>('PUT', '/api/settings/flugmappe', body)
        : apiSend<SettingsView>('DELETE', '/api/settings/flugmappe'),
    onSuccess: (saved) => {
      client.setQueryData(queryKeys.settings, saved);
      // Der Server schreibt die Adresse ohne Schrägstrich am Ende: so zeigt das Formular, was gilt.
      if (saved.flugmappe) form.setFieldsValue(saved.flugmappe);
    },
  });
  const current = m.data ? m.data.flugmappe : view;
  return (
    <Form<FlugmappeSettings>
      form={form}
      layout="vertical"
      className="sl-form"
      initialValues={view ?? EMPTY_FLUGMAPPE}
      onFinish={(v) => m.mutate({ client_id: v.client_id ?? '', allowed_origin: v.allowed_origin ?? '' })}
    >
      <p className="sl-form-hint sl-form-state">
        <span>Schnittstelle</span>
        {current ? <StatusChip ton="normal" wort="an" /> : <StatusChip ton="neutral" wort="aus" />}
      </p>
      <p className="sl-form-hint">
        Die Flugmappe legt darüber Einsätze an, verknüpft sie und schickt Abschnitte und Kartenfotos. Ihre
        Konten brauchen die Rolle Operator oder Verwaltung und müssen sich einmal in Suchlage angemeldet
        haben. Ein leeres Formular schaltet die Schnittstelle aus.
      </p>
      <Form.Item
        label="Client-ID"
        name="client_id"
        extra="Client-ID der Flugmappe aus Pocket ID – nicht die von Suchlage"
      >
        <Input autoComplete="off" />
      </Form.Item>
      <Form.Item
        label="Adresse der Flugmappe"
        name="allowed_origin"
        extra="Nur Schema, Host und Port, ohne Pfad"
      >
        <Input placeholder="https://uav.iuk-ue.de" autoComplete="off" />
      </Form.Item>
      <Result
        done={m.isSuccess}
        error={saveError(m.error)}
        text={m.data?.flugmappe ? 'Gespeichert.' : 'Ausgeschaltet.'}
      />
      <Space wrap>
        <Button type="primary" htmlType="submit" loading={m.isPending && m.variables !== null}>
          Flugmappe speichern
        </Button>
        {current && (
          <Button loading={m.isPending && m.variables === null} onClick={() => m.mutate(null)}>
            Ausschalten
          </Button>
        )}
      </Space>
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
