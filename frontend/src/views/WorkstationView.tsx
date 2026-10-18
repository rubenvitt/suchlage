import { Button } from 'antd';
import { useState } from 'react';
import { useMe } from '../api/queries';
import { useMissionScope } from '../missions/useMissionScope';
import { Segmentleiste } from '../components/instrument';
import { useServerNow } from '../live/LiveProvider';
import type { LiveState } from '../live/reducer';
import { SituationMap } from '../map/SituationMap';
import { useMapEditor } from '../map/useMapEditor';
import { usePhotoSelection } from '../photos/usePhotoSelection';
import { TelemetryStrip } from '../telemetry/TelemetryStrip';
import { EmptyState } from './EmptyState';
import { FlightPanel } from './FlightPanel';
import { gatewayName, type Focus } from './selectors';
import { useFocus } from './useFocus';
import { useLiveMapData } from './useLiveMapData';
import { VideoSlot } from './VideoSlot';

/** Was über der Karte steht, solange es nichts zu fliegen gibt. */
function emptyOf(state: LiveState, focus: Focus) {
  if (!state.ready)
    return <EmptyState title="Lage wird abgerufen" text="Die Live-Verbindung zum Server wird aufgebaut." />;
  if (!focus.gateway)
    return (
      <EmptyState
        title="Keine Fernsteuerung verbunden"
        text="In Pilot 2 unter Cloud-Dienst die Kopplungsadresse öffnen. Neue Fernsteuerungen koppelt die Verwaltung unter Einstellungen → Fernsteuerungen."
      />
    );
  if (!focus.gateway.aircraft_sn)
    return (
      <EmptyState
        title="Fluggerät aus"
        text={`Die Fernsteuerung ${gatewayName(focus.gateway)} ist verbunden. Sobald die M350 eingeschaltet ist, erscheint sie hier.`}
      />
    );
  return null;
}

/** Arbeitsplatz: Karte, rechts Livebild und Flug, unten die Instrumentenleiste. */
export function WorkstationView() {
  const { state, focus, choose } = useFocus();
  const serverNow = useServerNow();
  const { data: me } = useMe();
  const [follow, setFollow] = useState(true);
  const [swapped, setSwapped] = useState(false);
  const { scope, ready } = useMissionScope();
  const map = useLiveMapData(state, focus, scope, ready);
  const { gateways } = map;
  const canControl = me?.role === 'admin' || me?.role === 'operator';
  const editor = useMapEditor(map.elements, {
    enabled: canControl && ready,
    scope,
    onStartDrawing: () => setFollow(false),
  });
  const photo = usePhotoSelection(map.photos, me != null && me.role !== 'kiosk');
  const empty = emptyOf(state, focus);

  return (
    <div className="sl-work" data-swap={swapped && focus.gateway != null}>
      <div className="sl-work-map">
        <SituationMap
          {...map}
          serverNow={serverNow}
          thresholds={state.thresholds}
          follow={follow}
          onFollowChange={setFollow}
          {...editor.mapProps}
          {...photo.mapProps}
        />
        {empty && <div className="sl-view-notice">{empty}</div>}
        {editor.overlay}
        {photo.overlay}
      </div>
      <aside className="sl-work-side" aria-label="Livebild und Flug">
        {/* Ohne Fernsteuerung gibt es nichts zu wählen und kein Bild zum Vergrößern. */}
        {focus.gateway && (
          <div className="sl-work-tools">
            {gateways.length > 1 && (
              <Segmentleiste
                beschriftung="Fernsteuerung wählen"
                optionen={gateways.map((g) => ({ wert: g.sn, label: gatewayName(g) }))}
                wert={focus.gateway.sn}
                onWechsel={choose}
              />
            )}
            <Button
              size="small"
              className="sl-work-swap"
              aria-pressed={swapped}
              type={swapped ? 'primary' : 'default'}
              onClick={() => setSwapped((s) => !s)}
            >
              Livebild groß
            </Button>
          </div>
        )}
        <div className="sl-work-video">
          <VideoSlot focus={focus} ready={state.ready} canControl={canControl} />
        </div>
        {state.ready && <FlightPanel focus={focus} serverNow={serverNow} />}
      </aside>
      <div className="sl-work-strip">
        <TelemetryStrip
          aircraft={focus.aircraft}
          gateway={focus.gateway}
          flight={focus.flight?.flight ?? focus.ended}
          serverNow={serverNow}
          thresholds={state.thresholds}
          size="compact"
        />
      </div>
    </div>
  );
}
