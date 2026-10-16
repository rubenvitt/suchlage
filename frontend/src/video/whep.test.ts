import { http, HttpResponse } from 'msw';
import { server } from '../test/server';
import { WhepNotReady, startWhep } from './whep';

/** Ersatz für RTCPeerConnection, die jsdom nicht kennt. Kandidaten sind sofort gesammelt. */
class FakePeer {
  static last: FakePeer;
  iceGatheringState: RTCIceGatheringState = 'complete';
  connectionState: RTCPeerConnectionState = 'new';
  localDescription: RTCSessionDescriptionInit | null = null;
  remoteDescription: RTCSessionDescriptionInit | null = null;
  transceivers: string[] = [];
  closed = false;
  onconnectionstatechange: (() => void) | null = null;
  ontrack: unknown = null;
  constructor() {
    FakePeer.last = this;
  }
  addTransceiver(kind: string) {
    this.transceivers.push(kind);
  }
  async createOffer() {
    return { type: 'offer' as const, sdp: 'v=0 offer' };
  }
  async setLocalDescription(d: RTCSessionDescriptionInit) {
    this.localDescription = d;
  }
  async setRemoteDescription(d: RTCSessionDescriptionInit) {
    this.remoteDescription = d;
  }
  addEventListener() {}
  removeEventListener() {}
  close() {
    this.closed = true;
  }
  enter(state: RTCPeerConnectionState) {
    this.connectionState = state;
    this.onconnectionstatechange?.();
  }
}

const WHEP_URL = '/api/video/RC1/whep';
const callbacks = () => ({ onConnected: vi.fn(), onFailed: vi.fn() });
const answer = (status: number) =>
  server.use(http.post(WHEP_URL, () => new HttpResponse('v=0 answer', { status })));

beforeEach(() => vi.stubGlobal('RTCPeerConnection', FakePeer));
afterEach(() => vi.unstubAllGlobals());

test('schickt das Angebot als SDP und übernimmt die Antwort', async () => {
  let request: { type: string | null; body: string } | undefined;
  server.use(
    http.post(WHEP_URL, async ({ request: r }) => {
      request = { type: r.headers.get('content-type'), body: await r.text() };
      return new HttpResponse('v=0 answer', { status: 201, headers: { 'content-type': 'application/sdp' } });
    }),
  );
  const session = await startWhep(WHEP_URL, document.createElement('video'), callbacks());
  const pc = FakePeer.last;
  expect(request).toEqual({ type: 'application/sdp', body: 'v=0 offer' });
  expect(pc.transceivers).toEqual(['video', 'audio']);
  expect(pc.remoteDescription).toEqual({ type: 'answer', sdp: 'v=0 answer' });
  expect(pc.closed).toBe(false);
  session.close();
  expect(pc.closed).toBe(true);
});

test('meldet Verbindung und Abbruch der Peer-Verbindung', async () => {
  answer(201);
  const cb = callbacks();
  await startWhep(WHEP_URL, document.createElement('video'), cb);
  FakePeer.last.enter('connected');
  expect(cb.onConnected).toHaveBeenCalledTimes(1);
  FakePeer.last.enter('disconnected');
  expect(cb.onFailed).not.toHaveBeenCalled();
  FakePeer.last.enter('failed');
  expect(cb.onFailed).toHaveBeenCalledTimes(1);
});

// Review Focus 2: Pilot 2 sendet kurz nach dem Streamstart noch nicht – das ist kein Fehlversuch.
test('WHEP 404 heißt „noch nicht bereit“ und schließt die Verbindung', async () => {
  answer(404);
  await expect(startWhep(WHEP_URL, document.createElement('video'), callbacks())).rejects.toBeInstanceOf(
    WhepNotReady,
  );
  expect(FakePeer.last.closed).toBe(true);
});

test('andere WHEP-Fehler sind echte Fehlversuche', async () => {
  answer(502);
  const result = startWhep(WHEP_URL, document.createElement('video'), callbacks());
  await expect(result).rejects.toThrow('WHEP 502');
  await expect(result).rejects.not.toBeInstanceOf(WhepNotReady);
  expect(FakePeer.last.closed).toBe(true);
});

test('ohne Netz bleibt keine Peer-Verbindung offen', async () => {
  server.use(http.post(WHEP_URL, () => HttpResponse.error()));
  await expect(startWhep(WHEP_URL, document.createElement('video'), callbacks())).rejects.toThrow();
  expect(FakePeer.last.closed).toBe(true);
});
