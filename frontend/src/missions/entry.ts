import { useSyncExternalStore } from 'react';
import type { Mission } from '../contract/generated/Mission';
import type { Role } from '../contract/generated/Role';

/**
 * Was `/` zeigt: Das Großbild, ein laufender Einsatz, eine Adresse mit `?einsatz=` und ein
 * übersprungener Einstieg führen in die Lage, sonst kommt die Startseite.
 */
export function entryFor({
  role,
  missions,
  param,
  skipped,
}: {
  role: Role;
  missions: Mission[];
  param: string | null;
  skipped: boolean;
}): 'lage' | 'welcome' {
  if (role === 'kiosk' || param !== null || skipped) return 'lage';
  return missions.some((m) => m.state === 'running') ? 'lage' : 'welcome';
}

/** „Überspringen“ gilt für die Browsersitzung, damit Neuladen nicht wieder die Startseite zeigt. */
export const SKIP_KEY = 'suchlage:ohne-einsatz';

// Ohne Sitzungsspeicher (gesperrt) gilt die Wahl wenigstens, bis die Seite neu lädt.
let fallback = false;
const listeners = new Set<() => void>();

function read(): boolean {
  try {
    return sessionStorage.getItem(SKIP_KEY) === '1';
  } catch {
    return fallback;
  }
}

function write(skipped: boolean) {
  fallback = skipped;
  try {
    if (skipped) sessionStorage.setItem(SKIP_KEY, '1');
    else sessionStorage.removeItem(SKIP_KEY);
  } catch {
    // gesperrt: `fallback` trägt die Wahl
  }
  listeners.forEach((l) => l());
}

function subscribe(listener: () => void) {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

/** Geteilt über alle Komponenten: `reset()` im Kopf bringt den Einstieg zurück auf die Startseite. */
export function useSkip(): { skipped: boolean; skip(): void; reset(): void } {
  const skipped = useSyncExternalStore(subscribe, read);
  return { skipped, skip: () => write(true), reset: () => write(false) };
}
