import { Alert, Button, Form, Input } from 'antd';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { Link } from 'react-router';
import { ApiError, apiSend } from '../api/client';
import { queryKeys, useMissions } from '../api/queries';
import { Paneel } from '../components/instrument';
import type { Mission } from '../contract/generated/Mission';
import type { MissionCreate } from '../contract/generated/MissionCreate';
import type { MissionState } from '../contract/generated/MissionState';
import type { SectionCreate } from '../contract/generated/SectionCreate';
import { Frame } from '../shell/Frame';
import './missions.css';

// Der laufende Einsatz steht in der Kopfleiste; das Archiv zeigt die zuletzt abgeschlossenen zuerst.
const GROUPS: { state: MissionState; title: string }[] = [
  { state: 'prepared', title: 'Vorbereitet' },
  { state: 'closed', title: 'Archiv' },
];
const newestClosedFirst = (a: Mission, b: Mission) => (b.closed_at_ms ?? 0) - (a.closed_at_ms ?? 0);

type Act = (method: 'POST', path: string, body?: unknown, onOk?: () => void) => void;

function SectionForm({ mission, act }: { mission: Mission; act: Act }) {
  const [form] = Form.useForm<{ label: string }>();
  return (
    <Form
      form={form}
      layout="inline"
      onFinish={(v) => {
        act(
          'POST',
          `/api/missions/${mission.id}/sections`,
          {
            label: v.label,
            location: null,
          } satisfies SectionCreate,
          () => form.resetFields(),
        );
      }}
    >
      <Form.Item name="label" rules={[{ required: true, message: 'Name fehlt' }]}>
        <Input size="small" placeholder="Neuer Abschnitt" aria-label={`Abschnitt zu ${mission.name}`} />
      </Form.Item>
      <Button size="small" htmlType="submit" aria-label={`Abschnitt anlegen: ${mission.name}`}>
        Abschnitt anlegen
      </Button>
    </Form>
  );
}

function MissionRow({ mission, act }: { mission: Mission; act: Act }) {
  return (
    <li className="sl-mission">
      <div className="sl-mission-head">
        <strong>{mission.name}</strong>
        {mission.location && <span>{mission.location}</span>}
        <div className="sl-mission-actions">
          {mission.state === 'prepared' && (
            <Button
              size="small"
              type="primary"
              aria-label={`Starten: ${mission.name}`}
              onClick={() => act('POST', `/api/missions/${mission.id}/start`)}
            >
              Starten
            </Button>
          )}
          <Link to={`/?einsatz=${mission.id}`} aria-label={`Auf der Lage öffnen: ${mission.name}`}>
            Auf der Lage öffnen
          </Link>
        </div>
      </div>
      {mission.sections.length > 0 && (
        <ul className="sl-mission-sections">
          {mission.sections.map((s) => (
            <li key={s.id}>{s.label}</li>
          ))}
        </ul>
      )}
      {mission.state !== 'closed' && <SectionForm mission={mission} act={act} />}
    </li>
  );
}

export function MissionsPage() {
  const { data = [] } = useMissions();
  const client = useQueryClient();
  const [form] = Form.useForm<{ name: string; location?: string }>();
  const run = useMutation({
    mutationFn: (a: { path: string; body?: unknown }) => apiSend<unknown>('POST', a.path, a.body),
    onSuccess: () => client.invalidateQueries({ queryKey: queryKeys.missions }),
  });
  const act: Act = (_m, path, body, onOk) => run.mutate({ path, body }, { onSuccess: onOk });
  const error = run.error;
  return (
    <Frame>
      <div className="sl-page sl-missions">
        {error && (
          <Alert
            type="error"
            title={
              error instanceof ApiError
                ? error.status === 409 && run.variables?.path.endsWith('/start')
                  ? 'Es läuft schon ein Einsatz'
                  : error.message
                : 'Server nicht erreichbar.'
            }
          />
        )}
        <Paneel titel="Einsätze">
          <Form
            form={form}
            layout="inline"
            className="sl-create"
            colon={false}
            requiredMark={false}
            onFinish={(v) => {
              act(
                'POST',
                '/api/missions',
                {
                  name: v.name,
                  location: v.location || null,
                } satisfies MissionCreate,
                () => form.resetFields(),
              );
            }}
          >
            <Form.Item label="Name" name="name" rules={[{ required: true, message: 'Name fehlt' }]}>
              <Input placeholder="Vermisste Person Ebstorf" />
            </Form.Item>
            <Form.Item label="Ort" name="location">
              <Input />
            </Form.Item>
            <Button type="primary" htmlType="submit" loading={run.isPending}>
              Einsatz anlegen
            </Button>
          </Form>
        </Paneel>
        {GROUPS.map((g) => (
          <Paneel key={g.state} titel={g.title}>
            <ul className="sl-mission-list">
              {data
                .filter((m) => m.state === g.state)
                .sort(newestClosedFirst)
                .map((m) => (
                  <MissionRow key={m.id} mission={m} act={act} />
                ))}
            </ul>
          </Paneel>
        ))}
        <Paneel titel="Ohne Einsatz">
          <p>
            Was ohne laufenden Einsatz entstanden ist, lässt sich dort einem Einsatz zuordnen.{' '}
            <Link to="/?einsatz=ohne">Lage ohne Einsatz</Link> · <Link to="/flights?einsatz=ohne">Flüge ohne Einsatz</Link>
          </p>
        </Paneel>
      </div>
    </Frame>
  );
}
