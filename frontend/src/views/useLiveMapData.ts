import { useMemo } from 'react';
import type { LiveState } from '../live/reducer';
import type { Focus } from './selectors';

/**
 * Was die Lagekarte einer Live-Ansicht zeichnet. Die Arrays ändern sich nur mit neuen Daten: Der
 * Sekundentakt der Uhr allein baut die Spur nicht neu und unterbricht beim Folgen keinen Zoom.
 */
export function useLiveMapData(state: LiveState, focus: Focus) {
  const gateways = useMemo(() => Object.values(state.gateways), [state.gateways]);
  const aircraft = useMemo(() => (focus.aircraft ? [focus.aircraft] : []), [focus.aircraft]);
  const tracks = useMemo(() => (focus.flight ? [focus.flight.segments] : []), [focus.flight]);
  const elements = useMemo(() => Object.values(state.mapElements), [state.mapElements]);
  return { gateways, aircraft, tracks, elements };
}
