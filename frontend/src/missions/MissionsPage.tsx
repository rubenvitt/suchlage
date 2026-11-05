import { Alert, Button, Input } from 'antd';
import { Fragment, useMemo, useState } from 'react';
import { Link, useNavigate } from 'react-router';
import { ApiError } from '../api/client';
import { useMissionSummaries, useMissions, useStartMission } from '../api/queries';
import { useViewMode } from '../app/viewMode';
import { Kennzahl, Kennzahlenband, Paneel, Segmentleiste, StatusChip } from '../components/instrument';
import type { Mission } from '../contract/generated/Mission';
import type { MissionSummary } from '../contract/generated/MissionSummary';
import { useServerNow } from '../live/LiveProvider';
import { Frame } from '../shell/Frame';
import { MissionHeader } from './MissionHeader';
import {
  dateText,
  hoursMinutes,
  listGroup,
  listOrder,
  matchesSearch,
  missionDayMs,
  missionDurationMs,
  STATE_CHIP,
  timeText,
} from './missionInfo';
import { NewMissionDialog } from './NewMissionDialog';
import './missions.css';

type Filter = 'alle' | 'prepared' | 'closed';

const DASH = '–';

/** Kennzeichen, die über den Einsatz hinaus wirken: Flugmappe und Einsatztagebuch. */
export function MissionMarks({ mission }: { mission: Mission }) {
  const fm = mission.origin === 'flugmappe' || mission.flugmappe_links.length > 0;
  if (!fm && mission.etb_linked_at_ms == null) return null;
  return (
    <span className="sl-marks">
      {fm && <StatusChip ton="bedien" wort="Flugmappe" />}
      {mission.etb_linked_at_ms != null && <StatusChip ton="bedien" wort="Einsatztagebuch" />}
    </span>
  );
}

function Row({
  mission,
  summary,
  canStart,
  onStart,
  starting,
}: {
  mission: Mission;
  summary: MissionSummary | undefined;
  canStart: boolean;
  onStart: () => void;
  starting: boolean;
}) {
  const navigate = useNavigate();
  const chip = STATE_CHIP[mission.state];
  const duration = missionDurationMs(mission, 0);
  const open = `/einsaetze/${mission.id}`;
  return (
    // Die ganze Zeile öffnet den Einsatz; für Tastatur und Vorleser ist der Name der Link.
    <li className="sl-mrow" onClick={() => void navigate(open)}>
      <span className="sl-mrow-state">
        <StatusChip ton={chip.ton} wort={chip.wort} />
      </span>
      <span className="sl-mrow-name">
        <Link to={open} onClick={(e) => e.stopPropagation()}>
          {mission.name}
        </Link>
        <span>{mission.location ?? DASH}</span>
      </span>
      <span className="sl-mrow-nums">
        <span className="sl-mrow-num" data-label="Datum">
          {dateText(missionDayMs(mission))}
        </span>
        <span className="sl-mrow-num" data-label="Dauer">
          {mission.state === 'closed' && duration != null ? `${hoursMinutes(duration)} h` : DASH}
        </span>
        <span className="sl-mrow-num" data-label="Flüge">
          {summary?.flights ?? DASH}
        </span>
        <span className="sl-mrow-num" data-label="Fotos">
          {summary?.photos ?? DASH}
        </span>
      </span>
      <span className="sl-mrow-marks">
        <MissionMarks mission={mission} />
      </span>
      <span className="sl-mrow-action">
        {mission.state === 'prepared' && canStart && (
          <Button
            size="small"
            loading={starting}
            aria-label={`Starten: ${mission.name}`}
            onClick={(e) => {
              e.stopPropagation();
              onStart();
            }}
          >
            Starten
          </Button>
        )}
      </span>
    </li>
  );
}

function RunningPanel({ mission, summary }: { mission: Mission; summary: MissionSummary | undefined }) {
  const now = useServerNow(15_000);
  return (
    <Paneel
      titel="Laufender Einsatz"
      aktion={
        <Link className="sl-panel-link" to="/">
          Zur Lage
        </Link>
      }
    >
      <Link className="sl-running" to={`/einsaetze/${mission.id}`}>
        <span className="sl-running-name">{mission.name}</span>
        <span className="sl-running-meta">
          {[mission.location, mission.started_at_ms != null && `seit ${timeText(mission.started_at_ms)}`]
            .filter(Boolean)
            .join(' · ')}
        </span>
        <MissionMarks mission={mission} />
      </Link>
      <Kennzahlenband beschriftung="Laufender Einsatz in Zahlen">
        <Kennzahl titel="Dauer" wert={hoursMinutes(missionDurationMs(mission, now) ?? 0)} einheit="h" punkt="normal" />
        <Kennzahl
          titel="Flüge"
          wert={summary?.flights ?? DASH}
          notiz={summary ? `${hoursMinutes(summary.flight_ms)} h Flugzeit` : undefined}
        />
        <Kennzahl titel="Fotos" wert={summary?.photos ?? DASH} />
        <Kennzahl titel="Abschnitte" wert={mission.sections.length} />
      </Kennzahlenband>
    </Paneel>
  );
}

/**
 * Einsatzverwaltung (SUL-119): der laufende Einsatz mit Kennzahlen, darunter alle anderen mit Filter
 * und Suche, das Archiv nach Monaten. Jede Zeile führt auf die Einsatzseite.
 */
export function MissionsPage() {
  const missions = useMissions();
  const summaries = useMissionSummaries();
  const mobile = useViewMode() === 'mobile';
  const start = useStartMission();
  const [filter, setFilter] = useState<Filter>('alle');
  const [query, setQuery] = useState('');
  const [creating, setCreating] = useState(false);
  const all = useMemo(() => missions.data ?? [], [missions.data]);
  const running = all.find((m) => m.state === 'running');
  const byId = useMemo(() => new Map(summaries.data?.map((s) => [s.mission_id, s])), [summaries.data]);
  const others = all.filter((m) => m.state !== 'running');
  const shown = others
    .filter((m) => filter === 'alle' || m.state === filter)
    .filter((m) => matchesSearch(m, query))
    .sort(listOrder);
  const count = (state: Filter) => others.filter((m) => m.state === state).length;

  return (
    <Frame mission={!mobile && <MissionHeader />}>
      <div className="sl-page sl-missions">
        <div className="sl-title-row">
          <h1 className="sl-h1">Einsätze</h1>
          <Button type="primary" onClick={() => setCreating(true)}>
            Neuer Einsatz
          </Button>
        </div>
        {start.error && (
          <Alert
            type="error"
            title={
              start.error instanceof ApiError
                ? start.error.status === 409
                  ? 'Es läuft schon ein Einsatz'
                  : start.error.message
                : 'Server nicht erreichbar.'
            }
          />
        )}
        {missions.isError && <Alert type="error" title="Einsätze nicht geladen" />}

        {running && <RunningPanel mission={running} summary={byId.get(running.id)} />}

        <Paneel titel="Alle Einsätze" meta={missions.data ? `${shown.length} von ${others.length}` : undefined}>
          <div className="sl-mlist-tools">
            <Segmentleiste<Filter>
              beschriftung="Einsätze filtern"
              wert={filter}
              onWechsel={setFilter}
              optionen={[
                { wert: 'alle', label: 'Alle' },
                { wert: 'prepared', label: `Vorbereitet ${count('prepared')}` },
                { wert: 'closed', label: `Abgeschlossen ${count('closed')}` },
              ]}
            />
            <Input.Search
              allowClear
              className="sl-mlist-search"
              placeholder="Name oder Ort"
              aria-label="Einsätze durchsuchen"
              onChange={(e) => setQuery(e.target.value)}
            />
          </div>
          <ul className="sl-mlist" aria-label="Einsätze">
            <li className="sl-mrow sl-mrow-head" aria-hidden="true">
              <span>Status</span>
              <span>Einsatz</span>
              <span className="sl-mrow-nums">
                <span>Datum</span>
                <span>Dauer</span>
                <span>Flüge</span>
                <span>Fotos</span>
              </span>
              <span />
              <span />
            </li>
            {shown.map((m, i) => {
              const group = listGroup(m);
              return (
                <Fragment key={m.id}>
                  {(i === 0 || listGroup(shown[i - 1]!) !== group) && <li className="sl-mgroup">{group}</li>}
                  <Row
                    mission={m}
                    summary={byId.get(m.id)}
                    canStart={!running}
                    starting={start.isPending && start.variables === m.id}
                    onStart={() => start.mutate(m.id)}
                  />
                </Fragment>
              );
            })}
            {missions.data && shown.length === 0 && (
              <li className="sl-mlist-empty">{others.length === 0 ? 'Noch keine Einsätze' : 'Keine Treffer'}</li>
            )}
          </ul>
        </Paneel>

        <Paneel titel="Ohne Einsatz">
          <div className="sl-without">
            <Link to="/?einsatz=ohne">Lage ohne Einsatz</Link>
            <Link to="/flights?einsatz=ohne">Flüge ohne Einsatz</Link>
          </div>
        </Paneel>
      </div>
      {creating && <NewMissionDialog running={running !== undefined} onClose={() => setCreating(false)} />}
    </Frame>
  );
}
