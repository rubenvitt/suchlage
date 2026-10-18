import { useEffect } from 'react';
import { useMissionScope } from '../missions/useMissionScope';
import { useServerNow } from '../live/LiveProvider';
import { SituationMap } from '../map/SituationMap';
import { TelemetryStrip } from '../telemetry/TelemetryStrip';
import { useDichte } from '../theme/ThemeModeProvider';
import { useFocus } from './useFocus';
import { useLiveMapData } from './useLiveMapData';
import { useWakeLock } from './useWakeLock';
import { VideoSlot } from './VideoSlot';
import { viewerNoticeOf } from './viewerNotice';

/** Was aus der Entfernung zählt; das Alter steht immer dabei. */
const KIOSK_VALUES = ['elevation', 'speed', 'heading', 'battery', 'distance', 'mode', 'flight'];

/** Großbild: Karte und Livebild nebeneinander, darunter große Ziffern. Niemand bedient, die Karte folgt. */
export function KioskView() {
  useWakeLock();
  const { setDichte } = useDichte();
  // Aus der Entfernung gelesen: größere Abstände (UI-Richtung). Das Großbild-Gerät behält die Stufe.
  useEffect(() => setDichte('komfortabel'), [setDichte]);
  const { state, focus } = useFocus();
  const serverNow = useServerNow();
  // Das Großbild folgt immer dem laufenden Einsatz (`useMissionScope` ignoriert `?einsatz=`).
  const { scope, ready } = useMissionScope();
  const map = useLiveMapData(state, focus, scope, ready);
  const notice = viewerNoticeOf(state, focus);

  return (
    <div className="sl-kiosk">
      <div className="sl-kiosk-map">
        <SituationMap {...map} serverNow={serverNow} thresholds={state.thresholds} follow showTools={false} />
        {scope.kind === 'mission' && <div className="sl-kiosk-mission">{scope.mission.name}</div>}
        {notice && <div className="sl-view-notice">{notice}</div>}
      </div>
      <div className="sl-kiosk-video">
        <VideoSlot focus={focus} ready={state.ready} canControl={false} />
      </div>
      <div className="sl-kiosk-strip">
        <TelemetryStrip
          aircraft={focus.aircraft}
          gateway={focus.gateway}
          flight={focus.flight?.flight ?? focus.ended}
          serverNow={serverNow}
          thresholds={state.thresholds}
          size="large"
          only={KIOSK_VALUES}
        />
      </div>
    </div>
  );
}
