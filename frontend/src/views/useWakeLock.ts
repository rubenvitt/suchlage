import { useEffect } from 'react';

/**
 * Hält den Bildschirm wach, solange die Ansicht steht (Großbild). Der Browser gibt die Sperre selbst
 * frei, sobald die Seite verdeckt ist; wird sie wieder sichtbar, fordert der Hook sie neu an. Ohne
 * Wake Lock (ältere Browser, kein HTTPS) oder bei Ablehnung bleibt alles, wie es ist.
 */
export function useWakeLock() {
  useEffect(() => {
    if (!('wakeLock' in navigator)) return;
    let active = true;
    let lock: WakeLockSentinel | undefined;
    const request = () =>
      navigator.wakeLock
        .request('screen')
        .then((granted) => {
          // Kam die Sperre erst nach dem Verlassen der Ansicht, darf sie nicht stehen bleiben.
          if (active) lock = granted;
          else void granted.release();
        })
        .catch(() => {});
    void request();
    const onVisible = () => {
      if (document.visibilityState === 'visible') void request();
    };
    document.addEventListener('visibilitychange', onVisible);
    return () => {
      active = false;
      document.removeEventListener('visibilitychange', onVisible);
      void lock?.release();
    };
  }, []);
}
