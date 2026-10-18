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
 * Lage und bleiben in jeder Auswahl sichtbar. Solange die Einsätze unbekannt sind (`ready` falsch),
 * zeigt die Karte weder Elemente noch Fotos statt die von „ohne Einsatz“.
 */
export function useLiveMapData(state: LiveState, focus: Focus, scope: MissionScope, ready: boolean) {
  const gateways = useMemo(() => Object.values(state.gateways), [state.gateways]);
  const aircraft = useMemo(() => (focus.aircraft ? [focus.aircraft] : []), [focus.aircraft]);
  const tracks = useMemo(() => (focus.flight ? [focus.flight.segments] : []), [focus.flight]);
  const elements = useMemo(
    () => (ready ? Object.values(state.mapElements).filter((e) => inScope(e.mission_id, scope)) : []),
    [state.mapElements, scope, ready],
  );
  const { data: listed } = useMapPhotos(scope, ready);
  const photos = useMemo(
    () =>
      ready
        ? mergeShots(
            listed,
            Object.fromEntries(
              Object.entries(state.photoShots).filter(([, s]) => inScope(s.mission_id, scope)),
            ),
          )
        : [],
    [listed, state.photoShots, scope, ready],
  );
  return { gateways, aircraft, tracks, elements, photos };
}
