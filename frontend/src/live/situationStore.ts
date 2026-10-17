import type { LiveEvent } from '../contract/generated/LiveEvent';
import type { ConnectionStatus } from './connection';
import { applyEvent, initialLiveState, type LiveState } from './reducer';

/** Externer Speicher für `useSyncExternalStore`: Live-Stand und Verbindungsstatus. */
export function createLiveStore(now: () => number = () => Date.now()) {
  let state: LiveState = initialLiveState;
  let status: ConnectionStatus = 'connecting';
  const listeners = new Set<() => void>();
  const emit = () => listeners.forEach((l) => l());
  return {
    getState: () => state,
    getStatus: () => status,
    subscribe: (l: () => void) => {
      listeners.add(l);
      return () => {
        listeners.delete(l);
      };
    },
    dispatch: (e: LiveEvent) => {
      const next = applyEvent(state, e, now());
      if (next === state) return;
      state = next;
      emit();
    },
    setStatus: (s: ConnectionStatus) => {
      if (s === status) return;
      status = s;
      emit();
    },
  };
}

export type LiveStore = ReturnType<typeof createLiveStore>;
