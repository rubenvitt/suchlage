import type { LiveState } from '../live/reducer';
import { EmptyState } from './EmptyState';
import { gatewayName, type Focus } from './selectors';

/**
 * Was Großbild und mobil zeigen, solange es nichts zu sehen gibt. Zuschauer bedienen nichts, deshalb
 * ohne die Bedienschritte des Arbeitsplatzes: nur, was fehlt und wann es erscheint.
 */
export function viewerNoticeOf(state: LiveState, focus: Focus) {
  if (!state.ready)
    return <EmptyState title="Lage wird abgerufen" text="Die Live-Verbindung zum Server wird aufgebaut." />;
  if (!focus.gateway)
    return (
      <EmptyState
        title="Keine Drohne im Einsatz"
        text="Livebild und Position erscheinen, sobald eine Fernsteuerung verbunden ist."
      />
    );
  if (!focus.gateway.aircraft_sn)
    return (
      <EmptyState
        title="Fluggerät aus"
        text={`Die Fernsteuerung ${gatewayName(focus.gateway)} ist verbunden. Livebild und Position erscheinen, sobald die M350 eingeschaltet ist.`}
      />
    );
  return null;
}
