import { useMemo } from 'react';
import { useMapPhotos } from '../api/queries';
import type { LiveState } from '../live/reducer';
import { inScope, type MissionScope } from '../missions/scope';
import { mergeShots } from '../photos/photos';
import type { Focus } from './selectors';

/**
 * Was die Lagekarte einer Live-Ansicht zeichnet. Die Arrays ändern sich nur mit neuen Daten: Der
 * Sekundentakt der Uhr allein baut die Spur nicht neu und unterbricht beim Folgen keinen Zoom.
 * Elemente und Fotos gehören zum gewählten Einsatz; Fluggerät und laufende Spur sind die aktuelle
 * Lage und bleiben in jeder Auswahl sichtbar.
 */
export function useLiveMapData(state: LiveState, focus: Focus, scope: MissionScope) {
  const gateways = useMemo(() => Object.values(state.gateways), [state.gateways]);
  const aircraft = useMemo(() => (focus.aircraft ? [focus.aircraft] : []), [focus.aircraft]);
  const tracks = useMemo(() => (focus.flight ? [focus.flight.segments] : []), [focus.flight]);
  const elements = useMemo(
    () => Object.values(state.mapElements).filter((e) => inScope(e.mission_id, scope)),
    [state.mapElements, scope],
  );
  const { data: listed } = useMapPhotos(scope);
  const photos = useMemo(
    () =>
      mergeShots(
        listed,
        Object.fromEntries(Object.entries(state.photoShots).filter(([, s]) => inScope(s.mission_id, scope))),
      ),
    [listed, state.photoShots, scope],
  );
  return { gateways, aircraft, tracks, elements, photos };
}
