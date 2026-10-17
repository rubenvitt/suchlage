import { useViewMode } from '../app/viewMode';
import { Frame } from '../shell/Frame';
import { SyncCell } from '../shell/SyncCell';
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
      status={
        <>
          <SyncCell />
          <Context />
        </>
      }
    >
      {mode === 'workstation' && <WorkstationView />}
    </Frame>
  );
}
