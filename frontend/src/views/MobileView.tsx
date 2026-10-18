import { useState } from 'react';
import { useMe } from '../api/queries';
import { MissionBar } from '../missions/MissionBar';
import { useMissionScope } from '../missions/useMissionScope';
import { useServerNow } from '../live/LiveProvider';
import { SituationMap } from '../map/SituationMap';
import { usePhotoSelection } from '../photos/usePhotoSelection';
import { TelemetryStrip } from '../telemetry/TelemetryStrip';
import { useFocus } from './useFocus';
import { useLiveMapData } from './useLiveMapData';
import { VideoSlot } from './VideoSlot';
import { viewerNoticeOf } from './viewerNotice';

/** 2×4 Zellen: sieben Werte und das Alter. */
const MOBILE_VALUES = ['elevation', 'speed', 'battery', 'distance', 'gnss', 'mode', 'flight'];

/** Mobil nur zuschauen: Livebild oben, die wichtigsten Werte, darunter die Karte. */
export function MobileView() {
  const { state, focus } = useFocus();
  const serverNow = useServerNow();
  const { scope, ready } = useMissionScope();
  const map = useLiveMapData(state, focus, scope, ready);
  // Wer die Karte mit dem Finger verschiebt, beendet das Folgen; der Knopf holt es zurück.
  const [follow, setFollow] = useState(true);
  const notice = viewerNoticeOf(state, focus);
  const { data: me } = useMe();
  const photo = usePhotoSelection(map.photos, me != null && me.role !== 'kiosk');

  return (
    <div className="sl-mobile">
      <div className="sl-mobile-mission">
        <MissionBar />
      </div>
      {notice}
      {focus.gateway && <VideoSlot focus={focus} ready={state.ready} canControl={false} />}
      <TelemetryStrip
        aircraft={focus.aircraft}
        gateway={focus.gateway}
        flight={focus.flight?.flight ?? focus.ended}
        serverNow={serverNow}
        thresholds={state.thresholds}
        size="mobile"
        only={MOBILE_VALUES}
      />
      <div className="sl-mobile-map">
        <SituationMap
          {...map}
          serverNow={serverNow}
          thresholds={state.thresholds}
          follow={follow}
          onFollowChange={setFollow}
          {...photo.mapProps}
        />
        {photo.overlay}
      </div>
    </div>
  );
}
