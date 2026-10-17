import { act, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { http, HttpResponse } from 'msw';
import { liveEvents, snapshotFlying } from '../contract/fixtures.generated';
import type { StreamState } from '../contract/generated/StreamState';
import { server } from '../test/server';
import { renderWithApp } from '../test/render';
import { startHls } from './hls';
import { ICE_TIMEOUT_MS, RETRY_MS, STALL_MS } from './playerMachine';
import { WhepNotReady, startWhep } from './whep';
import { VideoPanel } from './VideoPanel';

// Nur startWhep ersetzen; WhepNotReady bleibt die echte Klasse.
vi.mock('./whep', async (importOriginal) => ({
  ...(await importOriginal<typeof import('./whep')>()),
  startWhep: vi.fn(),
}));
vi.mock('./hls', () => ({ startHls: vi.fn() }));

const pending = () => new Promise<never>(() => {});
const sources = snapshotFlying.gateways[0]!.video_sources;
const live = snapshotFlying.streams[0]!;
const failed = (liveEvents.find((e) => e.type === 'stream') as { stream: StreamState }).stream;
const idle: StreamState = { ...failed, phase: 'idle', error: null };

beforeEach(() => {
  vi.mocked(startWhep).mockReset().mockImplementation(pending);
  vi.mocked(startHls).mockReset().mockImplementation(pending);
});
afterEach(() => vi.useRealTimers());

/** Fängt den JSON-Körper eines Befehls an die API. */
function captureCommand(path: string) {
  const sent: unknown[] = [];
  server.use(
    http.post(path, async ({ request }) => {
      sent.push(await request.json());
      return HttpResponse.json(live);
    }),
  );
  return sent;
}

test('ohne Stream bietet die Bedienung den Start an und sendet ihn', async () => {
  const sent = captureCommand('/api/streams/start');
  renderWithApp(<VideoPanel gatewaySn="RCSIM0001" stream={idle} sources={sources} canControl />);
  expect(screen.getByText('Kein Livebild.')).toBeInTheDocument();
  await userEvent.click(screen.getByRole('button', { name: 'Livebild starten' }));
  await vi.waitFor(() => expect(sent).toEqual([{ gateway_sn: 'RCSIM0001', video_id: null }]));
  expect(startWhep).not.toHaveBeenCalled();
});

test('Großbild bekommt keinen Bedienknopf, aber eine Erklärung', () => {
  renderWithApp(<VideoPanel gatewaySn="RCSIM0001" stream={undefined} sources={sources} canControl={false} />);
  expect(screen.queryByRole('button')).not.toBeInTheDocument();
  expect(screen.getByText('Startet automatisch beim Abheben.')).toBeInTheDocument();
});

test('Ablehnung durch Pilot 2 wird im Wortlaut gezeigt', () => {
  renderWithApp(<VideoPanel gatewaySn="RCSIM0001" stream={failed} sources={sources} canControl />);
  expect(screen.getByText(/513011/)).toBeInTheDocument();
  expect(screen.getByRole('button', { name: 'Erneut versuchen' })).toBeInTheDocument();
});

test('während Pilot 2 startet oder beendet, gibt es keinen Startknopf', () => {
  const { rerender } = renderWithApp(
    <VideoPanel gatewaySn="RCSIM0001" stream={{ ...idle, phase: 'starting' }} sources={sources} canControl />,
  );
  expect(screen.getByText('Pilot 2 startet das Livebild …')).toBeInTheDocument();
  expect(screen.queryByRole('button')).not.toBeInTheDocument();
  rerender(
    <VideoPanel gatewaySn="RCSIM0001" stream={{ ...idle, phase: 'stopping' }} sources={sources} canControl />,
  );
  expect(screen.getByText('Livebild wird beendet …')).toBeInTheDocument();
  expect(screen.queryByRole('button')).not.toBeInTheDocument();
});

test('laufender Stream verbindet und zeigt die Linsenwahl', () => {
  renderWithApp(<VideoPanel gatewaySn="RCSIM0001" stream={live} sources={sources} canControl />);
  expect(screen.getByText('Verbindet …')).toBeInTheDocument();
  expect(startWhep).toHaveBeenCalledWith(
    '/api/video/RCSIM0001/whep',
    expect.any(HTMLVideoElement),
    expect.anything(),
  );
  expect(screen.getByRole('radiogroup', { name: 'Linse wählen' })).toBeInTheDocument();
  expect(screen.getByRole('radio', { name: 'Weit' })).toHaveAttribute('aria-checked', 'true');
  expect(screen.getByRole('radio', { name: 'Wärme' })).toBeInTheDocument();
});

test('verbundene Sitzung bleibt offen, wenn der Player auf „spielt“ wechselt', async () => {
  const close = vi.fn();
  vi.mocked(startWhep).mockImplementationOnce(async (_url, _video, cb) => {
    cb.onConnected();
    return { close };
  });
  renderWithApp(<VideoPanel gatewaySn="RCSIM0001" stream={live} sources={sources} canControl />);
  await vi.waitFor(() => expect(screen.queryByText('Verbindet …')).not.toBeInTheDocument());
  expect(screen.getByText('H20N · WebRTC')).toBeInTheDocument();
  expect(close).not.toHaveBeenCalled();
});

test('Linsenwahl schickt die Linse an die Fernsteuerung', async () => {
  const sent = captureCommand('/api/streams/lens');
  renderWithApp(<VideoPanel gatewaySn="RCSIM0001" stream={live} sources={sources} canControl />);
  await userEvent.click(screen.getByRole('radio', { name: 'Wärme' }));
  await vi.waitFor(() => expect(sent).toEqual([{ gateway_sn: 'RCSIM0001', lens: 'thermal' }]));
});

test('ein abgelehnter Befehl nennt den Grund vom Server', async () => {
  server.use(
    http.post('/api/streams/stop', () =>
      HttpResponse.json({ error: 'Broker nicht erreichbar.' }, { status: 503 }),
    ),
  );
  renderWithApp(<VideoPanel gatewaySn="RCSIM0001" stream={live} sources={sources} canControl />);
  await userEvent.click(screen.getByRole('button', { name: 'Livebild beenden' }));
  expect(await screen.findByRole('alert')).toHaveTextContent('Broker nicht erreichbar.');
});

// Review Focus 2: außerhalb des VPN verbindet ICE nie, der Player wechselt selbst auf HLS.
test('ohne ICE-Verbindung wechselt das Paneel nach der Frist auf HLS', async () => {
  vi.useFakeTimers();
  const close = vi.fn();
  vi.mocked(startWhep).mockResolvedValueOnce({ close });
  vi.mocked(startHls).mockImplementationOnce(async (_url, _video, cb) => {
    cb.onConnected();
    return { close: () => {} };
  });
  renderWithApp(<VideoPanel gatewaySn="RCSIM0001" stream={live} sources={sources} canControl={false} />);
  await act(() => vi.advanceTimersByTimeAsync(ICE_TIMEOUT_MS - 1));
  expect(startHls).not.toHaveBeenCalled();
  await act(() => vi.advanceTimersByTimeAsync(1));
  expect(close).toHaveBeenCalled();
  expect(startHls).toHaveBeenCalledWith(
    '/api/video/RCSIM0001/hls/index.m3u8',
    expect.any(HTMLVideoElement),
    expect.anything(),
  );
  expect(screen.getByText('H20N · verzögert (HLS)')).toBeInTheDocument();
});

// Review Focus 2: ein WHEP 404 kurz nach dem Streamstart ist kein Grund für den Rückfall.
test('solange Pilot 2 noch nicht sendet, bleibt das Paneel bei WebRTC', async () => {
  vi.useFakeTimers();
  vi.mocked(startWhep).mockRejectedValue(new WhepNotReady('WHEP 404'));
  renderWithApp(<VideoPanel gatewaySn="RCSIM0001" stream={live} sources={sources} canControl />);
  await act(() => vi.advanceTimersByTimeAsync(0));
  expect(screen.getByText('Noch kein Bild – neuer Versuch in 3 s')).toBeInTheDocument();
  for (let i = 0; i < 4; i++) await act(() => vi.advanceTimersByTimeAsync(RETRY_MS));
  expect(startWhep).toHaveBeenCalledTimes(5);
  expect(startHls).not.toHaveBeenCalled();
});

test('ein hängendes Bild bekommt die Schraffur und einen Neuversuch', async () => {
  vi.useFakeTimers();
  let fail = () => {};
  vi.mocked(startWhep).mockImplementationOnce(async (_url, _video, cb) => {
    cb.onConnected();
    fail = cb.onFailed;
    return { close: () => {} };
  });
  const { container } = renderWithApp(
    <VideoPanel gatewaySn="RCSIM0001" stream={live} sources={sources} canControl />,
  );
  await act(() => vi.advanceTimersByTimeAsync(0));
  act(() => fail());
  expect(screen.getByText('Bild hängt – neuer Versuch in 3 s')).toBeInTheDocument();
  expect(container.querySelector('.sl-video')).toHaveAttribute('data-player', 'stalled');
  await act(() => vi.advanceTimersByTimeAsync(RETRY_MS));
  expect(startWhep).toHaveBeenCalledTimes(2);
  // Auch der Neuversuch zeigt das hängende Bild, nicht „kein Bild“.
  expect(screen.getByText('Bild hängt – verbindet neu …')).toBeInTheDocument();
  expect(container.querySelector('.sl-video')).toHaveAttribute('data-player', 'stalled');
});

/** WebRTC-Sitzung, die sofort verbindet. */
function connectingWhep(close = () => {}) {
  vi.mocked(startWhep).mockImplementationOnce(async (_url, _video, cb) => {
    cb.onConnected();
    return { close };
  });
}

/** jsdom kennt `requestVideoFrameCallback` nicht; die Attrappe liefert Frames auf Zuruf. */
function stubFrames() {
  let next: VideoFrameRequestCallback = () => {};
  const proto = HTMLVideoElement.prototype as Partial<HTMLVideoElement>;
  proto.requestVideoFrameCallback = (cb) => ((next = cb), 1);
  proto.cancelVideoFrameCallback = () => {};
  onTestFinished(() => {
    delete proto.requestVideoFrameCallback;
    delete proto.cancelVideoFrameCallback;
  });
  return () => next(performance.now(), {} as VideoFrameCallbackMetadata);
}

const look = (container: HTMLElement) => container.querySelector('.sl-video')?.getAttribute('data-player');

// Ein WebRTC-Strom friert ohne `waiting` ein (VPN weg, Pilot 2 sendet nicht mehr); nur ausbleibende
// Frames verraten den Hänger.
test('kommen keine Frames mehr, bekommt das Bild die Schraffur', async () => {
  vi.useFakeTimers();
  const frame = stubFrames();
  connectingWhep();
  const { container } = renderWithApp(
    <VideoPanel gatewaySn="RCSIM0001" stream={live} sources={sources} canControl />,
  );
  await act(() => vi.advanceTimersByTimeAsync(STALL_MS - 1));
  frame();
  await act(() => vi.advanceTimersByTimeAsync(STALL_MS - 1));
  expect(look(container)).toBe('playing');
  await act(() => vi.advanceTimersByTimeAsync(1));
  expect(look(container)).toBe('stalled');
  expect(screen.getByText('Bild hängt – neuer Versuch in 3 s')).toBeInTheDocument();
});

test('ein verdeckter Tab malt keine Frames und gilt nicht als Hänger', async () => {
  vi.useFakeTimers();
  stubFrames();
  connectingWhep();
  const hidden = vi.spyOn(document, 'hidden', 'get').mockReturnValue(true);
  const { container } = renderWithApp(
    <VideoPanel gatewaySn="RCSIM0001" stream={live} sources={sources} canControl />,
  );
  await act(() => vi.advanceTimersByTimeAsync(3 * STALL_MS));
  expect(look(container)).toBe('playing');
  hidden.mockRestore();
  await act(() => vi.advanceTimersByTimeAsync(STALL_MS));
  expect(look(container)).toBe('stalled');
});

test('ohne Frame-Rückruf zählt langes Puffern als Hänger', async () => {
  vi.useFakeTimers();
  connectingWhep();
  const { container } = renderWithApp(
    <VideoPanel gatewaySn="RCSIM0001" stream={live} sources={sources} canControl />,
  );
  await act(() => vi.advanceTimersByTimeAsync(0));
  const el = container.querySelector('video')!;
  act(() => el.dispatchEvent(new Event('waiting')));
  await act(() => vi.advanceTimersByTimeAsync(STALL_MS - 1));
  act(() => el.dispatchEvent(new Event('playing')));
  await act(() => vi.advanceTimersByTimeAsync(STALL_MS));
  expect(look(container)).toBe('playing');
  act(() => el.dispatchEvent(new Event('waiting')));
  await act(() => vi.advanceTimersByTimeAsync(STALL_MS));
  expect(look(container)).toBe('stalled');
});

// Bei zwei gekoppelten Fernsteuerungen kann der Fokus wechseln, während beide senden.
test('wechselt die Fernsteuerung, verbindet das Paneel neu mit WebRTC', async () => {
  const close = vi.fn();
  connectingWhep(close);
  const { rerender } = renderWithApp(
    <VideoPanel gatewaySn="RCSIM0001" stream={live} sources={sources} canControl />,
  );
  expect(await screen.findByText('H20N · WebRTC')).toBeInTheDocument();
  rerender(<VideoPanel gatewaySn="RCSIM0002" stream={live} sources={sources} canControl />);
  expect(close).toHaveBeenCalledTimes(1);
  expect(startWhep).toHaveBeenCalledTimes(2);
  expect(startWhep).toHaveBeenLastCalledWith(
    '/api/video/RCSIM0002/whep',
    expect.any(HTMLVideoElement),
    expect.anything(),
  );
  expect(screen.getByText('Verbindet …')).toBeInTheDocument();
});

test('endet der Stream, wird die Sitzung geschlossen', async () => {
  const close = vi.fn();
  vi.mocked(startWhep).mockResolvedValueOnce({ close });
  const { rerender } = renderWithApp(
    <VideoPanel gatewaySn="RCSIM0001" stream={live} sources={sources} canControl />,
  );
  await vi.waitFor(() => expect(startWhep).toHaveBeenCalled());
  await act(() => Promise.resolve());
  rerender(<VideoPanel gatewaySn="RCSIM0001" stream={idle} sources={sources} canControl />);
  expect(close).toHaveBeenCalledTimes(1);
  expect(screen.getByRole('button', { name: 'Livebild starten' })).toBeInTheDocument();
});
