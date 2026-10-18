import { useViewMode } from '../app/viewMode';
import { MissionHeader } from '../missions/MissionHeader';
import { Frame } from '../shell/Frame';
import { SyncCell } from '../shell/SyncCell';
import { KioskView } from './KioskView';
import { MobileView } from './MobileView';
import { gatewayName } from './selectors';
import { useFocus } from './useFocus';
import './views.css';
import { WorkstationView } from './WorkstationView';

/** Im Kopf steht, wessen Lage gezeigt wird: dieselbe Fernsteuerung wie in der Ansicht. */
function Context() {
  const { gateway } = useFocus().focus;
  if (!gateway) return null;
  return (
    <span className="sl-context">
      {gatewayName(gateway)}
      {gateway.aircraft_sn && ' · M350'}
    </span>
  );
}

/** Die Lage; die Ansicht folgt dem Gerät. Den Live-Strom stellt `SessionLayout`. */
export function LagePage() {
  const mode = useViewMode();
  return (
    <Frame
      // Mobil steht der Einsatz über der Ansicht (`MobileView`), im Kopf ist dafür kein Platz.
      mission={mode !== 'mobile' && <MissionHeader />}
      status={
        <>
          <SyncCell />
          {/* Mobil reicht der Platz im Kopf nur für den Live-Strom (UI-Richtung). */}
          {mode !== 'mobile' && <Context />}
        </>
      }
    >
      {mode === 'workstation' && <WorkstationView />}
      {mode === 'kiosk' && <KioskView />}
      {mode === 'mobile' && <MobileView />}
    </Frame>
  );
}
