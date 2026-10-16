import { useMutation } from '@tanstack/react-query';
import { Button } from 'antd';
import { useRef } from 'react';
import { ApiError, apiSend } from '../api/client';
import { Paneel } from '../components/instrument';
import type { Lens } from '../contract/generated/Lens';
import type { StreamLensRequest } from '../contract/generated/StreamLensRequest';
import type { StreamStartRequest } from '../contract/generated/StreamStartRequest';
import type { StreamState } from '../contract/generated/StreamState';
import type { StreamStopRequest } from '../contract/generated/StreamStopRequest';
import type { VideoSource } from '../contract/generated/VideoSource';
import { playerMessage, streamMessage, TRANSPORT_LABEL } from './copy';
import { LensPicker } from './LensPicker';
import { usePlayer } from './usePlayer';
import './video.css';

interface Props {
  gatewaySn: string;
  stream: StreamState | undefined;
  sources: VideoSource[];
  /** Bedienung darf starten, beenden und die Linse wählen; Großbild und mobil schauen nur zu. */
  canControl: boolean;
}

type Command =
  | { path: '/api/streams/start'; body: StreamStartRequest }
  | { path: '/api/streams/stop'; body: StreamStopRequest }
  | { path: '/api/streams/lens'; body: StreamLensRequest };

const PENDING = 'Befehl unterwegs …';

export function VideoPanel({ gatewaySn, stream, sources, canControl }: Props) {
  const video = useRef<HTMLVideoElement>(null);
  const isLive = stream?.phase === 'live';
  const player = usePlayer(video, gatewaySn, isLive);

  // Das Ergebnis kommt als `stream`-Ereignis über die Live-Verbindung, nicht aus der Antwort.
  const command = useMutation({ mutationFn: ({ path, body }: Command) => apiSend('POST', path, body) });
  const error =
    command.error && (command.error instanceof ApiError ? command.error.message : 'Befehl nicht gesendet.');
  const start = () =>
    command.mutate({ path: '/api/streams/start', body: { gateway_sn: gatewaySn, video_id: null } });
  const stop = () => command.mutate({ path: '/api/streams/stop', body: { gateway_sn: gatewaySn } });
  const lens = (l: Lens) =>
    command.mutate({ path: '/api/streams/lens', body: { gateway_sn: gatewaySn, lens: l } });

  const source = sources.find((s) => s.video_id === stream?.video_id);
  const meta = [source?.camera_label, player.kind === 'playing' ? TRANSPORT_LABEL[player.transport] : null]
    .filter(Boolean)
    .join(' · ');
  const overlay = isLive ? playerMessage(player) : streamMessage(stream, canControl);
  const look = player.kind === 'retrying' && player.hadPicture ? 'stalled' : player.kind;
  const busy = stream?.phase === 'starting' || stream?.phase === 'stopping';

  const actions = canControl && !busy && (
    <div className="sl-video-actions">
      {isLive ? (
        <>
          <LensPicker
            lenses={source?.switchable_lenses ?? []}
            value={stream?.lens ?? source?.lens ?? null}
            onChange={lens}
            disabled={command.isPending ? PENDING : undefined}
          />
          <Button onClick={stop} disabled={command.isPending}>
            Livebild beenden
          </Button>
        </>
      ) : (
        <Button type="primary" onClick={start} loading={command.isPending}>
          {stream?.phase === 'failed' ? 'Erneut versuchen' : 'Livebild starten'}
        </Button>
      )}
      {(error || command.isPending) && (
        <p className="sl-video-note" role={error ? 'alert' : 'status'}>
          {error || PENDING}
        </p>
      )}
    </div>
  );

  return (
    <Paneel titel="Livebild" meta={meta || undefined} fuss={actions || undefined}>
      <div className="sl-video" data-player={look}>
        <video ref={video} muted autoPlay playsInline aria-label="Livebild der Drohne" />
        {overlay && (
          <div className="sl-video-overlay" role="status">
            {overlay}
          </div>
        )}
      </div>
    </Paneel>
  );
}
