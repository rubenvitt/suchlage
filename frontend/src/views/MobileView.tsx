import { useState } from 'react';
import { useServerNow } from '../live/LiveProvider';
import { SituationMap } from '../map/SituationMap';
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
  const map = useLiveMapData(state, focus);
  // Wer die Karte mit dem Finger verschiebt, beendet das Folgen; der Knopf holt es zurück.
  const [follow, setFollow] = useState(true);
  const notice = viewerNoticeOf(state, focus);

  return (
    <div className="sl-mobile">
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
        />
      </div>
    </div>
  );
}
