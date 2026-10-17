import { startHls } from './hls';

/** Ersatz für hls.js: jsdom hat keine Media Source Extensions. */
const fakeHls = vi.hoisted(() => {
  type Listener = (event: string, data: { fatal: boolean }) => void;
  class FakeHls {
    static supported = true;
    static last: FakeHls | undefined;
    static Events = { ERROR: 'hlsError' };
    static isSupported = () => FakeHls.supported;
    config: unknown;
    source: string | undefined;
    media: HTMLMediaElement | undefined;
    destroyed = false;
    private listeners: Listener[] = [];
    constructor(config: unknown) {
      this.config = config;
      FakeHls.last = this;
    }
    on(_event: string, listener: Listener) {
      this.listeners.push(listener);
    }
    loadSource(url: string) {
      this.source = url;
    }
    attachMedia(media: HTMLMediaElement) {
      this.media = media;
    }
    destroy() {
      this.destroyed = true;
      this.listeners = [];
    }
    emitError(fatal: boolean) {
      this.listeners.forEach((l) => l(FakeHls.Events.ERROR, { fatal }));
    }
  }
  return FakeHls;
});
vi.mock('hls.js', () => ({ default: fakeHls }));

const HLS_URL = '/api/video/RC1/hls/index.m3u8';

function video({ native }: { native: boolean }) {
  const el = document.createElement('video');
  el.canPlayType = () => (native ? 'maybe' : '');
  el.play = () => Promise.resolve();
  el.load = vi.fn(); // jsdom kennt load() nicht
  return el;
}

const callbacks = () => ({ onConnected: vi.fn(), onFailed: vi.fn() });

beforeEach(() => {
  fakeHls.supported = true;
  fakeHls.last = undefined;
});

describe('natives HLS (Safari, iOS)', () => {
  test('native HLS reports errors: ein Fehler am Video-Element führt zum Neuversuch', async () => {
    const el = video({ native: true });
    const cb = callbacks();
    const session = await startHls(HLS_URL, el, cb);
    expect(el.getAttribute('src')).toBe(HLS_URL);
    el.dispatchEvent(new Event('error'));
    expect(cb.onFailed).toHaveBeenCalledTimes(1);
    session.close();
    el.dispatchEvent(new Event('error'));
    expect(cb.onFailed).toHaveBeenCalledTimes(1);
  });

  test('nach close meldet das Element nichts mehr und gibt die Quelle frei', async () => {
    const el = video({ native: true });
    const cb = callbacks();
    const session = await startHls(HLS_URL, el, cb);
    session.close();
    el.dispatchEvent(new Event('playing'));
    el.dispatchEvent(new Event('error'));
    expect(cb.onConnected).not.toHaveBeenCalled();
    expect(cb.onFailed).not.toHaveBeenCalled();
    expect(el.hasAttribute('src')).toBe(false);
    expect(el.load).toHaveBeenCalled();
    expect(fakeHls.last).toBeUndefined();
  });

  test('das erste laufende Bild meldet die Verbindung', async () => {
    const el = video({ native: true });
    const cb = callbacks();
    await startHls(HLS_URL, el, cb);
    el.dispatchEvent(new Event('playing'));
    expect(cb.onConnected).toHaveBeenCalledTimes(1);
  });
});

describe('hls.js', () => {
  test('lädt die Playlist, meldet nur fatale Fehler und räumt beim Schließen auf', async () => {
    const el = video({ native: false });
    const cb = callbacks();
    const session = await startHls(HLS_URL, el, cb);
    const hls = fakeHls.last!;
    expect(hls.source).toBe(HLS_URL);
    expect(hls.media).toBe(el);
    hls.emitError(false);
    expect(cb.onFailed).not.toHaveBeenCalled();
    hls.emitError(true);
    expect(cb.onFailed).toHaveBeenCalledTimes(1);
    session.close();
    expect(hls.destroyed).toBe(true);
    el.dispatchEvent(new Event('playing'));
    expect(cb.onConnected).not.toHaveBeenCalled();
  });

  test('ohne Media Source Extensions scheitert der Versuch sofort', async () => {
    fakeHls.supported = false;
    const cb = callbacks();
    await startHls(HLS_URL, video({ native: false }), cb);
    expect(cb.onFailed).toHaveBeenCalledTimes(1);
    expect(fakeHls.last).toBeUndefined();
  });
});

// Ein WebRTC-Strom am Element hat Vorrang vor `src`: ohne Freigabe bliebe der Rückfall schwarz.
test('HLS löst einen verbliebenen WebRTC-Strom vom Element', async () => {
  for (const native of [true, false]) {
    const el = video({ native });
    el.srcObject = {} as MediaStream;
    await startHls(HLS_URL, el, callbacks());
    expect(el.srcObject).toBeNull();
  }
});
