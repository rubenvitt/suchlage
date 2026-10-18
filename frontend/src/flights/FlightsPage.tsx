import { Paneel, PaneelZustand, type PaneelDatenzustand } from '../components/instrument';
import { useFlights, useMissions } from '../api/queries';
import { useViewMode } from '../app/viewMode';
import { MissionBar } from '../missions/MissionBar';
import { useMissionScope } from '../missions/useMissionScope';
import { useServerNow } from '../live/LiveProvider';
import { Frame } from '../shell/Frame';
import { FlightCards } from './FlightCards';
import { FlightTable } from './FlightTable';

export function FlightsPage() {
  const { scope, ready } = useMissionScope();
  const missions = useMissions();
  const { data, isPending, isError, refetch } = useFlights(scope, ready);
  const mobile = useViewMode() === 'mobile';
  const nowMs = useServerNow();
  // Ohne bekannte Einsätze steht noch nicht fest, welche Flüge gemeint sind.
  const missionsFailed = !ready && missions.isError;
  const zustand: PaneelDatenzustand = missionsFailed
    ? 'fehler'
    : isPending
      ? 'laden'
      : isError
        ? 'fehler'
        : !data.length
          ? 'leer'
          : 'daten';
  const scopeName = scope.kind === 'mission' ? scope.mission.name : 'ohne Einsatz';
  const leerText =
    scope.kind === 'none'
      ? 'Keine Flüge ohne Einsatz.'
      : scope.mission.state === 'running'
        ? 'Noch kein Flug aufgezeichnet. Jeder Flug erscheint hier automatisch nach dem Abheben.'
        : 'Keine Flüge in diesem Einsatz.';
  return (
    <Frame>
      <div className="sl-page">
        <MissionBar />
        <Paneel titel="Flüge" meta={data ? `${scopeName} · ${data.length} aufgezeichnet` : undefined}>
          {/* Ohne Aktion: Flüge entstehen von selbst, ein Knopf führte ins Leere. */}
          <PaneelZustand
            zustand={zustand}
            titel="Flüge"
            leerText={leerText}
            onNeuladen={() => void (missionsFailed ? missions.refetch() : refetch())}
          >
            {data &&
              (mobile ? (
                <FlightCards flights={data} nowMs={nowMs} assignable={scope.kind === 'none'} />
              ) : (
                <FlightTable flights={data} nowMs={nowMs} assignable={scope.kind === 'none'} />
              ))}
          </PaneelZustand>
        </Paneel>
      </div>
    </Frame>
  );
}
