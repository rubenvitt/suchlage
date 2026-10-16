export interface MediaSession {
  close(): void;
}

export interface MediaCallbacks {
  onConnected: () => void;
  onFailed: () => void;
}

/** MediaMTX kennt den Pfad noch nicht (WHEP 404): Pilot 2 hat noch nicht zu senden begonnen. */
export class WhepNotReady extends Error {}

const GATHERING_MAX_MS = 2_000;

function gathered(pc: RTCPeerConnection): Promise<void> {
  if (pc.iceGatheringState === 'complete') return Promise.resolve();
  return new Promise((resolve) => {
    const done = () => {
      window.clearTimeout(timer);
      pc.removeEventListener('icegatheringstatechange', check);
      resolve();
    };
    const check = () => pc.iceGatheringState === 'complete' && done();
    const timer = window.setTimeout(done, GATHERING_MAX_MS);
    pc.addEventListener('icegatheringstatechange', check);
  });
}

/** WHEP ohne Trickle-ICE: alle Kandidaten im Offer, die Signalisierung läuft über den Server-Proxy. */
export async function startWhep(
  url: string,
  video: HTMLVideoElement,
  cb: MediaCallbacks,
): Promise<MediaSession> {
  const pc = new RTCPeerConnection();
  try {
    pc.addTransceiver('video', { direction: 'recvonly' });
    pc.addTransceiver('audio', { direction: 'recvonly' });
    pc.ontrack = (e) => {
      video.srcObject = e.streams[0] ?? new MediaStream([e.track]);
      void video.play().catch(() => {});
    };
    pc.onconnectionstatechange = () => {
      if (pc.connectionState === 'connected') cb.onConnected();
      if (pc.connectionState === 'failed') cb.onFailed();
    };
    await pc.setLocalDescription(await pc.createOffer());
    await gathered(pc);
    const res = await fetch(url, {
      method: 'POST',
      credentials: 'same-origin',
      headers: { 'content-type': 'application/sdp' },
      body: pc.localDescription!.sdp,
    });
    if (!res.ok) throw res.status === 404 ? new WhepNotReady('WHEP 404') : new Error(`WHEP ${res.status}`);
    await pc.setRemoteDescription({ type: 'answer', sdp: await res.text() });
  } catch (e) {
    // Jeder Neuversuch baut eine eigene Verbindung auf; eine gescheiterte darf nicht offen bleiben.
    pc.close();
    throw e;
  }
  return { close: () => pc.close() };
}
