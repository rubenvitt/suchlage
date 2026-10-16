import { connectLive, type ConnectionStatus } from './connection';

class FakeSource {
  static last: FakeSource;
  onopen: (() => void) | null = null;
  onmessage: ((e: { data: string }) => void) | null = null;
  onerror: (() => void) | null = null;
  readyState = 0;
  closed = false;
  constructor(public url: string) {
    FakeSource.last = this;
  }
  close() {
    this.closed = true;
  }
}

const create = (u: string) => new FakeSource(u) as unknown as EventSource;

afterEach(() => vi.useRealTimers());

test('meldet Status, reicht Ereignisse weiter und überspringt kaputte Nachrichten', () => {
  const statuses: ConnectionStatus[] = [];
  const events: unknown[] = [];
  const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
  const stop = connectLive({
    url: '/api/live',
    onEvent: (e) => events.push(e),
    onStatus: (s) => statuses.push(s),
    create,
  });
  const src = FakeSource.last;
  expect(src.url).toBe('/api/live');
  src.onopen?.();
  src.onmessage?.({
    data: '{"type":"system","system":{"broker":"connected","unknown_fields":[],"invalid_messages":0,"raw_recording":null}}',
  });
  src.onmessage?.({ data: 'kaputt' });
  src.onerror?.();
  expect(statuses).toEqual(['connecting', 'open', 'reconnecting']);
  expect(events).toHaveLength(1);
  expect(warn).toHaveBeenCalledOnce();
  expect(src.closed).toBe(false);
  stop();
  expect(src.closed).toBe(true);
  warn.mockRestore();
});

test('Fehler beim Verarbeiten gelten nicht als unlesbare Nachricht', () => {
  const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
  connectLive({
    url: '/api/live',
    onEvent: () => {
      throw new TypeError('Verarbeitung');
    },
    onStatus: () => {},
    create,
  });
  expect(() => FakeSource.last.onmessage?.({ data: '{"type":"system"}' })).toThrow('Verarbeitung');
  expect(warn).not.toHaveBeenCalled();
  warn.mockRestore();
});

test('baut die Verbindung neu auf, wenn der Browser aufgibt', () => {
  vi.useFakeTimers();
  const stop = connectLive({ url: '/api/live', onEvent: () => {}, onStatus: () => {}, create });
  const first = FakeSource.last;
  first.readyState = 2; // CLOSED
  first.onerror?.();
  expect(first.closed).toBe(true);
  vi.advanceTimersByTime(2_999);
  expect(FakeSource.last).toBe(first);
  vi.advanceTimersByTime(1);
  expect(FakeSource.last).not.toBe(first);
  stop();
  expect(FakeSource.last.closed).toBe(true);
});

test('nach dem Beenden kein neuer Versuch mehr', () => {
  vi.useFakeTimers();
  const stop = connectLive({ url: '/api/live', onEvent: () => {}, onStatus: () => {}, create });
  const first = FakeSource.last;
  first.readyState = 2;
  first.onerror?.();
  stop();
  vi.advanceTimersByTime(10_000);
  expect(FakeSource.last).toBe(first);
});
