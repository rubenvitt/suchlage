import { useQueryClient } from '@tanstack/react-query';
import { createContext, useContext, useEffect, useState, useSyncExternalStore, type ReactNode } from 'react';
import { queryKeys } from '../api/queries';
import type { LiveEvent } from '../contract/generated/LiveEvent';
import { connectLive } from './connection';
import { demoFeed } from './demoFeed';
import type { LiveState } from './reducer';
import { createLiveStore, type LiveStore } from './situationStore';

const Ctx = createContext<LiveStore | null>(null);

/** `store` für Tests (vorbefüllt, ohne Verbindung); sonst verbindet der Provider selbst. */
export function LiveProvider({ store, url = '/api/live', children }: { store?: LiveStore; url?: string; children: ReactNode }) {
  const [own] = useState(() => store ?? createLiveStore());
  const client = useQueryClient();
  useEffect(() => {
    if (store) return;
    const onEvent = (e: LiveEvent) => {
      own.dispatch(e);
      // Die Flugliste kennt neue und beendete Flüge, ohne zu pollen.
      if (e.type === 'flight_started' || e.type === 'flight_ended') {
        void client.invalidateQueries({ queryKey: queryKeys.flights });
      }
    };
    // Vorschau-Modus (`mise run ui`): abgespielter Flug statt SSE. Im Build ist MODE 'production', der Zweig fällt weg.
    const connect = import.meta.env.MODE === 'demo' ? demoFeed : connectLive;
    return connect({ url, onEvent, onStatus: own.setStatus });
  }, [own, store, url, client]);
  return <Ctx.Provider value={own}>{children}</Ctx.Provider>;
}

function useStore(): LiveStore {
  const s = useContext(Ctx);
  if (!s) throw new Error('useLive außerhalb von <LiveProvider>');
  return s;
}

/** Der Selektor muss vorhandene Referenzen zurückgeben (kein neues Objekt je Aufruf). */
export function useLive<T>(selector: (s: LiveState) => T): T {
  const s = useStore();
  return useSyncExternalStore(s.subscribe, () => selector(s.getState()));
}

export function useConnectionStatus() {
  const s = useStore();
  return useSyncExternalStore(s.subscribe, s.getStatus);
}

/** Serverzeit, sekündlich fortgeschrieben – Grundlage aller Frische-Anzeigen. */
export function useServerNow(intervalMs = 1000): number {
  const offset = useLive((s) => s.serverOffsetMs);
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const id = window.setInterval(() => setNow(Date.now()), intervalMs);
    return () => window.clearInterval(id);
  }, [intervalMs]);
  return now + offset;
}
