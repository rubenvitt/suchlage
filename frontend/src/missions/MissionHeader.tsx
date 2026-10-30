import { Alert, Button, Dropdown, Form, Input, Modal, message, type MenuProps } from 'antd';
import { useEffect, useState } from 'react';
import { Link, useLocation, useNavigate, useSearchParams } from 'react-router';
import { ApiError } from '../api/client';
import {
  deletedHere,
  useAddSection,
  useCloseMission,
  useMe,
  useMissions,
  useReleaseFlugmappe,
  useStartMission,
  useUpdateMission,
} from '../api/queries';
import { StatusChip, useRollen } from '../components/instrument';
import { statusFlaeche } from '../components/instrument/statusFlaeche';
import type { FlugmappeLink } from '../contract/generated/FlugmappeLink';
import type { Mission } from '../contract/generated/Mission';
import { useServerNow } from '../live/LiveProvider';
import { DeleteMissionDialog } from './DeleteMissionDialog';
import { useSkip } from './entry';
import {
  activeLinks,
  FlugmappeLinks,
  flugmappeHint,
  MapPhotoThumb,
  SECTIONS_FROM_FLUGMAPPE,
} from './flugmappe';
import './missions.css';
import { useMissionScope } from './useMissionScope';

const errorText = (e: unknown) => (e instanceof ApiError ? e.message : 'Server nicht erreichbar.');
const pad = (n: number) => String(n).padStart(2, '0');
/** Dauer seit dem Start als „01:12 h“. */
const since = (startMs: number | null, nowMs: number) => {
  const min = Math.max(0, Math.floor((nowMs - (startMs ?? nowMs)) / 60_000));
  return `${pad(Math.floor(min / 60))}:${pad(min % 60)} h`;
};

/**
 * Der Einsatz in der Kopfleiste: laufend mit Menü, ohne Einsatz mit Weg zur Startseite, Vorbereitung,
 * Archiv (nur lesen). Das Großbild zeigt nur den Namen des laufenden. Solange die Einsätze nicht
 * bekannt sind, steht hier nichts, was „ohne Einsatz“ behaupten würde.
 */
export function MissionHeader() {
  const { data: me } = useMe();
  const missions = useMissions();
  const { scope, ready } = useMissionScope();
  const param = useSearchParams()[0].get('einsatz');
  const { pathname } = useLocation();
  const navigate = useNavigate();
  // Gelöscht oder unbekannt (Spec §4 „Offene Ansichten“): zurück zum Einstieg dieser Seite statt
  // stillschweigend den laufenden Einsatz zu zeigen. Das Großbild kennt keine Adresse.
  const gone =
    ready &&
    me?.role !== 'kiosk' &&
    param !== null &&
    param !== 'ohne' &&
    !deletedHere.has(Number(param)) &&
    !missions.data?.some((m) => String(m.id) === param);
  useEffect(() => {
    if (gone) void navigate(pathname, { replace: true });
  }, [gone, navigate, pathname]);
  if (!me || gone) return null;
  if (me.role === 'kiosk')
    return scope.kind === 'mission' ? (
      <div className="sl-mission-status">
        <strong>{scope.mission.name}</strong>
      </div>
    ) : null;
  if (!ready)
    return (
      <div className="sl-mission-status">
        <span>{missions.isError ? 'Einsätze nicht geladen' : 'Einsätze werden geladen …'}</span>
      </div>
    );
  const admin = me.role === 'admin';
  if (scope.kind === 'none')
    return <Without running={missions.data?.some((m) => m.state === 'running') ?? false} />;
  const { mission } = scope;
  if (mission.state === 'running') return <Running mission={mission} />;
  if (mission.state === 'prepared') return <Prepared mission={mission} admin={admin} />;
  return <Archived mission={mission} admin={admin} />;
}

/**
 * Deutlich (Spec §2, Skizze 3): Niemand soll ohne Einsatz fliegen, ohne es zu merken.
 * `running`: Es läuft einer (Adresse `?einsatz=ohne`), dann führt der Knopf dorthin.
 */
function Without({ running }: { running: boolean }) {
  const { reset } = useSkip();
  const navigate = useNavigate();
  // Die Statusfläche trägt ihren Text in beiden Darstellungen, auch auf dem immer dunklen Kopf.
  const tint = statusFlaeche(useRollen().rollen, 'achtung');
  return (
    <div className="sl-mission-status" data-state="ohne" style={{ background: tint.grund, color: tint.text }}>
      <strong>Ohne Einsatz</strong>
      <span>Zeichnungen werden nach 24 h gelöscht</span>
      <Button
        type="primary"
        size="small"
        onClick={() => {
          reset();
          navigate('/');
        }}
      >
        {running ? 'Zum laufenden Einsatz' : 'Einsatz starten'}
      </Button>
    </div>
  );
}

/** „Löschen …“ mit Rückfrage, nur für die Verwaltung (vorbereitete und abgeschlossene Einsätze). */
function DeleteAction({ mission }: { mission: Mission }) {
  const [deleting, setDeleting] = useState(false);
  return (
    <>
      <Button size="small" danger onClick={() => setDeleting(true)}>
        Löschen …
      </Button>
      {deleting && <DeleteMissionDialog mission={mission} onClose={() => setDeleting(false)} />}
    </>
  );
}

function Prepared({ mission, admin }: { mission: Mission; admin: boolean }) {
  const start = useStartMission();
  const navigate = useNavigate();
  const [notice, holder] = message.useMessage();
  // Nach dem Start wechselt die Kopfleiste in „laufend“ und diese Komponente ist weg: darum das Promise.
  const run = () =>
    start.mutateAsync(mission.id).then(
      () => navigate('/'),
      (e: unknown) =>
        void notice.error(
          e instanceof ApiError && e.status === 409 ? 'Es läuft schon ein Einsatz' : errorText(e),
        ),
    );
  return (
    <div className="sl-mission-status" data-state="vorbereitung">
      {holder}
      <span>Vorbereitung:</span> <strong>{mission.name}</strong>
      <FlugmappeShort mission={mission} />
      <Button type="primary" size="small" loading={start.isPending} onClick={() => void run()}>
        Einsatz starten
      </Button>
      {admin && <DeleteAction mission={mission} />}
      <Link to="/">Zurück</Link>
    </div>
  );
}

function Archived({ mission, admin }: { mission: Mission; admin: boolean }) {
  return (
    <div className="sl-mission-status" data-state="archiv">
      <span>Archiv:</span> <strong>{mission.name}</strong> <span>· abgeschlossen</span>
      <FlugmappeShort mission={mission} />
      <Link to={`/flights?einsatz=${mission.id}`}>Flüge</Link>
      {admin && <DeleteAction mission={mission} />}
      <Link to="/einsaetze">Zurück</Link>
    </div>
  );
}

/**
 * „· Flugmappe“ im Kopf, der ganze Hinweis als Tooltip. Schmal ausgeblendet; dort nennt ihn beim
 * laufenden Einsatz das Menü.
 */
function FlugmappeShort({ mission }: { mission: Mission }) {
  const hint = flugmappeHint(mission);
  return hint ? (
    <span className="sl-mission-note sl-mission-fm-short" title={hint}>
      · Flugmappe
    </span>
  ) : null;
}

type Dialog = 'section' | 'edit' | 'close' | 'map-photos' | 'flugmappen';

function Running({ mission }: { mission: Mission }) {
  const now = useServerNow(15_000);
  const navigate = useNavigate();
  const [dialog, setDialog] = useState<Dialog | null>(null);
  const [open, setOpen] = useState(false);
  const section = mission.sections.at(-1);
  const hint = flugmappeHint(mission);
  const linked = activeLinks(mission).length > 0;
  const items: MenuProps['items'] = [
    {
      type: 'group',
      key: 'einsatz',
      label: 'Einsatz',
      children: [
        // Verknüpft: Abschnitte legt die Flugmappe an (Server: 422).
        ...(linked ? [] : [{ key: 'section', label: 'Neuer Abschnitt …' }]),
        ...(mission.sections.some((s) => s.has_map_photo) ? [{ key: 'map-photos', label: 'Kartenfotos …' }] : []),
        { key: 'edit', label: 'Einsatz bearbeiten' },
        { key: 'flights', label: 'Flüge dieses Einsatzes' },
        { key: 'close', label: 'Einsatz abschließen …', danger: true },
      ],
    },
    // Herkunft und Verknüpfung (schmal steht das nicht im Kopf), dazu die Flugmappen mit „Lösen“.
    ...(hint || mission.flugmappe_links.length
      ? [
          {
            type: 'group' as const,
            key: 'flugmappe',
            label: 'Flugmappe',
            children: [
              ...(hint ? [{ key: 'fm-hint', label: hint, disabled: true }] : []),
              ...(mission.flugmappe_links.length ? [{ key: 'flugmappen', label: 'Verknüpfte Flugmappen …' }] : []),
              ...(linked ? [{ key: 'fm-sections', label: SECTIONS_FROM_FLUGMAPPE, disabled: true }] : []),
            ],
          },
        ]
      : []),
  ];
  const onClick: MenuProps['onClick'] = ({ key }) => {
    setOpen(false);
    if (key === 'flights') void navigate(`/flights?einsatz=${mission.id}`);
    else setDialog(key as Dialog);
  };
  const close = () => setDialog(null);
  return (
    <div className="sl-mission-status" data-state="laufend">
      <Dropdown menu={{ items, onClick }} trigger={['click']} open={open} onOpenChange={setOpen}>
        <button type="button" className="sl-mission-trigger" aria-haspopup="menu" aria-expanded={open}>
          <strong>{mission.name}</strong>
          <StatusChip ton="normal" wort={`Laufend · ${since(mission.started_at_ms, now)}`} />
          {section && <span className="sl-mission-note">· Abschnitt {section.label}</span>}
          <FlugmappeShort mission={mission} />
          <span aria-hidden="true">▾</span>
        </button>
      </Dropdown>
      {dialog === 'section' && <SectionDialog mission={mission} onClose={close} />}
      {dialog === 'map-photos' && <MapPhotosDialog mission={mission} onClose={close} />}
      {dialog === 'flugmappen' && <FlugmappenDialog mission={mission} onClose={close} />}
      {dialog === 'edit' && <EditDialog mission={mission} onClose={close} />}
      {dialog === 'close' && <CloseDialog mission={mission} onClose={close} />}
    </div>
  );
}

function SectionDialog({ mission, onClose }: { mission: Mission; onClose: () => void }) {
  const add = useAddSection();
  const [form] = Form.useForm<{ label: string }>();
  return (
    <Modal
      open
      title="Neuer Abschnitt"
      okText="Anlegen"
      cancelText="Abbrechen"
      confirmLoading={add.isPending}
      onOk={() => form.submit()}
      onCancel={onClose}
    >
      <Form
        form={form}
        layout="vertical"
        requiredMark={false}
        onFinish={(v) =>
          void add.mutateAsync({ id: mission.id, label: v.label, location: null }).then(onClose, () => {})
        }
      >
        <Form.Item label="Abschnitt" name="label" rules={[{ required: true, message: 'Name fehlt' }]}>
          <Input autoFocus placeholder="z. B. Süd" />
        </Form.Item>
      </Form>
      {add.isError && <Alert type="error" title={errorText(add.error)} />}
    </Modal>
  );
}

/**
 * Verknüpfte Flugmappen mit Zustand. „Lösen“ ist der Ausweg bei einem verlorenen Gerät: Die Flugmappe
 * darf danach nichts mehr am Einsatz, bis sie neu verknüpft; ihre Flugbucheinträge bleiben.
 */
function FlugmappenDialog({ mission, onClose }: { mission: Mission; onClose: () => void }) {
  const release = useReleaseFlugmappe();
  const [asking, setAsking] = useState<FlugmappeLink | null>(null);
  return (
    <Modal open title={`Flugmappen · ${mission.name}`} footer={null} onCancel={onClose}>
      <p className="sl-mission-fm-sections">
        Der Einsatz schließt aus der Flugmappe, wenn keine verknüpfte mehr offen ist.
      </p>
      <FlugmappeLinks
        mission={mission}
        onRelease={setAsking}
        releasing={release.isPending ? (release.variables?.linkId ?? null) : null}
      />
      {asking && (
        <Alert
          type="warning"
          title={`Flugmappe ${asking.aircraft_label} lösen?`}
          description="Sie kann danach nichts mehr an diesem Einsatz ändern, bis sie neu verknüpft. Gemeldete Flüge bleiben."
          action={
            <span className="sl-fm-release-actions">
              <Button size="small" onClick={() => setAsking(null)}>
                Abbrechen
              </Button>
              <Button
                size="small"
                danger
                loading={release.isPending}
                onClick={() =>
                  void release.mutateAsync({ id: mission.id, linkId: asking.id }).then(
                    () => setAsking(null),
                    () => {},
                  )
                }
              >
                Lösen
              </Button>
            </span>
          }
        />
      )}
      {release.isError && <Alert type="error" title={errorText(release.error)} />}
    </Modal>
  );
}

/** Kartenfotos der Abschnitte (aus der Flugmappe), neueste Abschnitte zuletzt wie überall. */
function MapPhotosDialog({ mission, onClose }: { mission: Mission; onClose: () => void }) {
  return (
    <Modal open title={`Kartenfotos · ${mission.name}`} footer={null} onCancel={onClose}>
      <p className="sl-mission-fm-sections">Ein Klick öffnet das Foto in voller Größe in einem neuen Tab.</p>
      <ul className="sl-map-photos">
        {mission.sections
          .filter((s) => s.has_map_photo)
          .map((s) => (
            <li key={s.id}>
              <span>Abschnitt {s.label}</span>
              <MapPhotoThumb missionId={mission.id} section={s} />
            </li>
          ))}
      </ul>
    </Modal>
  );
}

function EditDialog({ mission, onClose }: { mission: Mission; onClose: () => void }) {
  const update = useUpdateMission();
  const [form] = Form.useForm<{ name: string; location?: string | null }>();
  return (
    <Modal
      open
      title="Einsatz bearbeiten"
      okText="Speichern"
      cancelText="Abbrechen"
      confirmLoading={update.isPending}
      onOk={() => form.submit()}
      onCancel={onClose}
    >
      <Form
        form={form}
        layout="vertical"
        requiredMark={false}
        initialValues={{ name: mission.name, location: mission.location }}
        onFinish={(v) =>
          void update
            .mutateAsync({ id: mission.id, name: v.name, location: v.location || null })
            .then(onClose, () => {})
        }
      >
        <Form.Item label="Name" name="name" rules={[{ required: true, message: 'Name fehlt' }]}>
          <Input />
        </Form.Item>
        <Form.Item label="Ort" name="location">
          <Input />
        </Form.Item>
      </Form>
      {update.isError && <Alert type="error" title={errorText(update.error)} />}
    </Modal>
  );
}

function CloseDialog({ mission, onClose }: { mission: Mission; onClose: () => void }) {
  const close = useCloseMission();
  const { reset } = useSkip();
  const navigate = useNavigate();
  return (
    <Modal
      open
      title={`Einsatz „${mission.name}“ abschließen?`}
      okText="Abschließen"
      cancelText="Abbrechen"
      okButtonProps={{ danger: true }}
      confirmLoading={close.isPending}
      // Danach ist die Kopfleiste schon „Archiv“ und dieser Dialog weg: darum das Promise. Ein früheres
      // „Überspringen“ gilt nicht mehr, nach dem Abschluss kommt die Startseite.
      onOk={() =>
        void close.mutateAsync(mission.id).then(() => {
          reset();
          navigate('/');
        }, () => {})
      }
      onCancel={onClose}
    >
      <p>Abgeschlossene Einsätze sind nur noch lesbar.</p>
      {close.isError && <Alert type="error" title={errorText(close.error)} />}
    </Modal>
  );
}
