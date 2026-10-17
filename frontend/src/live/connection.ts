import type { LiveEvent } from '../contract/generated/LiveEvent';

export type ConnectionStatus = 'connecting' | 'open' | 'reconnecting';

const RETRY_MS = 3_000;
/** `EventSource.CLOSED`; als Zahl, weil Tests (jsdom) keine `EventSource` kennen. */
const CLOSED = 2;

export interface LiveConnectionOptions {
  url: string;
  onEvent: (e: LiveEvent) => void;
  onStatus: (s: ConnectionStatus) => void;
  create?: (url: string) => EventSource;
}

/**
 * Hält genau eine EventSource offen. Der Browser verbindet selbst neu; gibt er auf (CLOSED), folgt
 * nach 3 s ein neuer Versuch. Nach jedem Verbinden schickt der Server zuerst einen Snapshot.
 */
export function connectLive({
  url,
  onEvent,
  onStatus,
  create = (u) => new EventSource(u),
}: LiveConnectionOptions): () => void {
  let source: EventSource | null = null;
  let timer: ReturnType<typeof setTimeout> | undefined;
  let stopped = false;

  const open = () => {
    onStatus('connecting');
    const src = create(url);
    source = src;
    src.onopen = () => onStatus('open');
    src.onmessage = (m) => {
      let event: LiveEvent;
      try {
        event = JSON.parse(m.data) as LiveEvent;
      } catch {
        console.warn('Live-Nachricht nicht lesbar – übersprungen');
        return;
      }
      onEvent(event);
    };
    src.onerror = () => {
      onStatus('reconnecting');
      if (src.readyState === CLOSED && !stopped) {
        src.close();
        timer = setTimeout(open, RETRY_MS);
      }
    };
  };

  open();
  return () => {
    stopped = true;
    clearTimeout(timer);
    source?.close();
  };
}
