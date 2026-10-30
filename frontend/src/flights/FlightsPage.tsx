import { Paneel, PaneelZustand, type PaneelDatenzustand } from '../components/instrument';
import { useFlights, useMissionFlights, useMissions } from '../api/queries';
import { useViewMode } from '../app/viewMode';
import { MissionHeader } from '../missions/MissionHeader';
import { useMissionScope } from '../missions/useMissionScope';
import { useServerNow } from '../live/LiveProvider';
import { Frame } from '../shell/Frame';
import { FlightCards } from './FlightCards';
import { FlightTable } from './FlightTable';
import { MissionFlightCards, MissionFlightTable } from './MissionFlightList';

/**
 * Flüge eines Einsatzes (Telemetrie und Handeinträge der Flugmappen, SUL-113) oder die aufgezeichneten
 * Flüge ohne Einsatz zum Zuordnen.
 */
export function FlightsPage() {
  const { scope, ready } = useMissionScope();
  const missions = useMissions();
  const missionId = ready && scope.kind === 'mission' ? scope.mission.id : null;
  const unassigned = useFlights(scope, ready && scope.kind === 'none');
  const ofMission = useMissionFlights(missionId);
  const { data, isPending, isError, refetch } = scope.kind === 'mission' ? ofMission : unassigned;
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
  const meta =
    data &&
    (scope.kind === 'mission'
      ? `${scope.mission.name} · ${data.length} ${data.length === 1 ? 'Flug' : 'Flüge'}`
      : `ohne Einsatz · ${data.length} aufgezeichnet`);
  const leerText =
    scope.kind === 'none'
      ? 'Keine Flüge ohne Einsatz.'
      : scope.mission.state === 'running'
        ? 'Noch kein Flug. Jeder Flug erscheint hier automatisch nach dem Abheben oder sobald ihn eine Flugmappe meldet.'
        : 'Keine Flüge in diesem Einsatz.';
  return (
    <Frame mission={!mobile && <MissionHeader />}>
      <div className="sl-page">
        {mobile && (
          <div className="sl-mobile-mission">
            <MissionHeader />
          </div>
        )}
        <Paneel titel="Flüge" meta={meta || undefined}>
          {/* Ohne Aktion: Flüge entstehen von selbst, ein Knopf führte ins Leere. */}
          <PaneelZustand
            zustand={zustand}
            titel="Flüge"
            leerText={leerText}
            onNeuladen={() => void (missionsFailed ? missions.refetch() : refetch())}
          >
            {scope.kind === 'mission' && ofMission.data
              ? mobile
                ? <MissionFlightCards flights={ofMission.data} nowMs={nowMs} />
                : <MissionFlightTable flights={ofMission.data} nowMs={nowMs} />
              : scope.kind === 'none' &&
                unassigned.data &&
                (mobile ? (
                  <FlightCards flights={unassigned.data} nowMs={nowMs} assignable />
                ) : (
                  <FlightTable flights={unassigned.data} nowMs={nowMs} assignable />
                ))}
          </PaneelZustand>
        </Paneel>
      </div>
    </Frame>
  );
}
