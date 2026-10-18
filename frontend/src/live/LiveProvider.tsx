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
      // Flugliste und Flugdetail (['flights', id]) kennen neue und beendete Flüge, ohne zu pollen. Ein
      // Snapshot kommt nach jedem Neuverbinden: Ein Flugstart oder eine Landung in der Lücke fehlte sonst.
      if (e.type === 'snapshot' || e.type === 'flight_started' || e.type === 'flight_ended') {
        void client.invalidateQueries({ queryKey: queryKeys.flights });
      }
      // Fotos aus einer Lücke im Strom holt die Liste nach; neue Fotos eines Flugs auch sein Detail.
      if (e.type === 'snapshot') void client.invalidateQueries({ queryKey: queryKeys.photos });
      if (e.type === 'missions_changed') void client.invalidateQueries({ queryKey: queryKeys.missions });
      if (e.type === 'photo_shot' && e.shot.flight_id != null) {
        void client.invalidateQueries({ queryKey: queryKeys.flightPhotos(e.shot.flight_id) });
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

/**
 * Übernimmt eine Antwort des Servers sofort in die Lage (etwa ein neues Kartenelement), statt auf
 * den Live-Strom zu warten. Der Strom bestätigt sie danach; doppelt angewendet ändert sie nichts.
 */
export function useLiveDispatch(): (e: LiveEvent) => void {
  return useStore().dispatch;
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
