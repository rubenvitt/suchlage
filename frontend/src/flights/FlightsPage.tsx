import { Paneel, PaneelZustand, type PaneelDatenzustand } from '../components/instrument';
import { useFlights } from '../api/queries';
import { useViewMode } from '../app/viewMode';
import { MissionBar } from '../missions/MissionBar';
import { useMissionScope } from '../missions/useMissionScope';
import { useServerNow } from '../live/LiveProvider';
import { Frame } from '../shell/Frame';
import { FlightCards } from './FlightCards';
import { FlightTable } from './FlightTable';

export function FlightsPage() {
  const { scope } = useMissionScope();
  const { data, isPending, isError, refetch } = useFlights(scope);
  const mobile = useViewMode() === 'mobile';
  const nowMs = useServerNow();
  const zustand: PaneelDatenzustand = isPending
    ? 'laden'
    : isError
      ? 'fehler'
      : !data.length
        ? 'leer'
        : 'daten';
  return (
    <Frame>
      <div className="sl-page">
        <MissionBar />
        <Paneel titel="Flüge" meta={data ? `${data.length} aufgezeichnet` : undefined}>
          {/* Ohne Aktion: Flüge entstehen von selbst, ein Knopf führte ins Leere. */}
          <PaneelZustand
            zustand={zustand}
            titel="Flüge"
            leerText="Noch kein Flug aufgezeichnet. Jeder Flug erscheint hier automatisch nach dem Abheben."
            onNeuladen={() => void refetch()}
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
