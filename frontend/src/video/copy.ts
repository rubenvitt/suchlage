import type { Lens } from '../contract/generated/Lens';
import type { StreamState } from '../contract/generated/StreamState';
import { RETRY_MS, type PlayerState, type Transport } from './playerMachine';

export const LENS_LABEL: Record<Lens, string> = {
  normal: 'Normal',
  wide: 'Weit',
  zoom: 'Zoom',
  thermal: 'Wärme',
};

export const TRANSPORT_LABEL: Record<Transport, string> = { webrtc: 'WebRTC', hls: 'verzögert (HLS)' };

/** Was mit dem Livebild der Fernsteuerung los ist, solange es nicht läuft. */
export function streamMessage(stream: StreamState | undefined, canControl: boolean): string {
  switch (stream?.phase) {
    case 'starting':
      return 'Pilot 2 startet das Livebild …';
    case 'stopping':
      return 'Livebild wird beendet …';
    case 'failed':
      return stream.error ?? 'Livebild ließ sich nicht starten.';
    case 'live':
      return '';
    default:
      return canControl ? 'Kein Livebild.' : 'Startet automatisch beim Abheben.';
  }
}

/** Was der Player gerade tut, solange kein Bild läuft. */
export function playerMessage(player: PlayerState): string {
  const retry = `neuer Versuch in ${RETRY_MS / 1000} s`;
  switch (player.kind) {
    case 'connecting':
      return 'Verbindet …';
    case 'retrying':
      return player.hadPicture ? `Bild hängt – ${retry}` : `Noch kein Bild – ${retry}`;
    default:
      return '';
  }
}
