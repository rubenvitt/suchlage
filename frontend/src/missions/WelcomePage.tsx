import { Alert, Button, Form, Input, message } from 'antd';
import { Link, useNavigate, useSearchParams } from 'react-router';
import { ApiError } from '../api/client';
import { useCreateMission, useMe, useMissions, useStartMission } from '../api/queries';
import { Paneel, StatusChip } from '../components/instrument';
import type { Mission } from '../contract/generated/Mission';
import { Frame } from '../shell/Frame';
import { SyncCell } from '../shell/SyncCell';
import { LagePage } from '../views/LagePage';
import { entryFor, useSkip } from './entry';
import './missions.css';

const isConflict = (e: unknown) => e instanceof ApiError && e.status === 409;
const errorText = (e: unknown) => (e instanceof ApiError ? e.message : 'Server nicht erreichbar.');
const created = (m: Mission) =>
  new Date(m.created_at_ms).toLocaleString('de-DE', {
    day: '2-digit',
    month: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
  });

/**
 * Startseite ohne laufenden Einsatz: neuen anlegen und gleich starten, einen vorbereiteten starten
 * oder überspringen. Startet ein Einsatz, wechselt der Einstieg von selbst in seine Lage.
 * `onConflict`: Lief schon einer (409), zeigt der Einstieg dessen Lage und meldet das hiermit.
 */
function WelcomePage({ onConflict }: { onConflict: (text: string) => void }) {
  const { data: missions = [] } = useMissions();
  const { skip } = useSkip();
  const navigate = useNavigate();
  const create = useCreateMission();
  const start = useStartMission();
  const [form] = Form.useForm<{ name: string; location?: string }>();
  const prepared = missions.filter((m) => m.state === 'prepared');
  const closed = missions.filter((m) => m.state === 'closed').length;
  const error = [create.error, start.error].find((e) => e && !isConflict(e));

  // Die Seite ist nach einem 409 schon weg: Die Meldung geht an den Einstieg, der bleibt. Eigener
  // Text, weil der Server hier nur den allgemeinen Konfliktsatz der Datenbank schickt.
  const conflict = (name: string) => (e: unknown) => {
    if (isConflict(e)) onConflict(`Es läuft schon ein Einsatz. „${name}“ ist vorbereitet, nicht gestartet.`);
  };
  const send = (v: { name: string; location?: string }, run: boolean) =>
    create
      .mutateAsync({ input: { name: v.name, location: v.location || null }, start: run })
      .then((m) => !run && navigate(`/?einsatz=${m.id}`))
      .catch(conflict(v.name));

  return (
    <Frame status={<SyncCell />}>
      <div className="sl-welcome">
        <section className="sl-welcome-new">
          <h1>Kein Einsatz läuft</h1>
          <p>Starte einen Einsatz, damit Flüge, Fotos und Suchgebiete zusammenbleiben.</p>
          {error && <Alert type="error" title={errorText(error)} />}
          <Paneel titel="Neuer Einsatz" koerperPolster>
            <Form
              form={form}
              layout="vertical"
              requiredMark={false}
              onFinish={(v) => void send(v, true)}
            >
              <Form.Item label="Einsatzname" name="name" rules={[{ required: true, message: 'Name fehlt' }]}>
                <Input size="large" placeholder="Vermisste Person Ebstorf" />
              </Form.Item>
              <Form.Item label="Ort (optional)" name="location">
                <Input size="large" placeholder="z. B. Waldgebiet Hanstedt" />
              </Form.Item>
              <Button type="primary" size="large" block htmlType="submit" loading={create.isPending}>
                Einsatz starten
              </Button>
              <Button
                size="large"
                block
                className="sl-welcome-prepare"
                disabled={create.isPending}
                onClick={() =>
                  void form.validateFields().then(
                    (v) => send(v, false),
                    () => {}, // die Felder zeigen den Fehler
                  )
                }
              >
                Nur vorbereiten
              </Button>
            </Form>
          </Paneel>
        </section>
        <section className="sl-welcome-side">
          <Paneel titel="Vorbereitet">
            {prepared.length === 0 ? (
              <p className="sl-welcome-empty">Keine vorbereiteten Einsätze.</p>
            ) : (
              <ul className="sl-mission-list">
                {prepared.map((m) => (
                  <li key={m.id} className="sl-mission">
                    <div className="sl-mission-head">
                      <StatusChip ton="achtung" wort="Vorbereitet" />
                      <div className="sl-welcome-item">
                        <Link to={`/?einsatz=${m.id}`}>{m.name}</Link>
                        <span>{[m.location, `angelegt ${created(m)}`].filter(Boolean).join(' · ')}</span>
                      </div>
                      <Button
                        aria-label={`Starten: ${m.name}`}
                        loading={start.isPending && start.variables === m.id}
                        onClick={() => void start.mutateAsync(m.id).catch(conflict(m.name))}
                      >
                        Starten
                      </Button>
                    </div>
                  </li>
                ))}
              </ul>
            )}
          </Paneel>
          {closed > 0 && (
            <p>
              Abgeschlossene Einsätze: <Link to="/einsaetze">Archiv öffnen ({closed})</Link>
            </p>
          )}
          <div className="sl-welcome-skip">
            Nur kurz fliegen, ohne Einsatz?{' '}
            <Button type="link" onClick={skip}>
              Überspringen
            </Button>
            <small>Zeichnungen ohne Einsatz werden nach 24 Stunden gelöscht.</small>
          </div>
        </section>
      </div>
    </Frame>
  );
}

/**
 * Route `/`: Lage oder Startseite nach `entryFor`. Das Großbild wartet nicht auf die Einsätze.
 * Scheitert ihr erster Abruf, bleibt die Lage erreichbar; ihre Einsatzleiste sagt, dass sie fehlen.
 */
export function EntryPage() {
  const { data: me } = useMe();
  const missions = useMissions();
  const [params] = useSearchParams();
  const { skipped } = useSkip();
  const [notice, holder] = message.useMessage();
  if (!me) return null;
  const page =
    missions.data === undefined && me.role !== 'kiosk'
      ? missions.isError
        ? 'lage'
        : null
      : entryFor({ role: me.role, missions: missions.data ?? [], param: params.get('einsatz'), skipped });
  return (
    <>
      {holder}
      {page === 'lage' && <LagePage />}
      {page === 'welcome' && <WelcomePage onConflict={(text) => void notice.warning(text)} />}
    </>
  );
}
