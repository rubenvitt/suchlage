import { Paneel } from '../components/instrument';
import { VideoPanel } from '../video/VideoPanel';
import type { Focus } from './selectors';

/** Das Livebild der Fernsteuerung im Fokus; ohne Fernsteuerung bleibt das Feld beschriftet. */
export function VideoSlot({
  focus,
  ready,
  canControl,
}: {
  focus: Focus;
  ready: boolean;
  canControl: boolean;
}) {
  if (focus.gateway)
    return (
      <VideoPanel
        gatewaySn={focus.gateway.sn}
        stream={focus.stream}
        sources={focus.gateway.video_sources}
        canControl={canControl}
      />
    );
  return (
    <Paneel titel="Livebild" koerperPolster>
      <p className="sl-muted">{ready ? 'Ohne Fernsteuerung kein Livebild.' : 'Wird abgerufen …'}</p>
    </Paneel>
  );
}
