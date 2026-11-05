import type { StatusTon } from '../components/instrument';
import type { Mission } from '../contract/generated/Mission';
import type { MissionState } from '../contract/generated/MissionState';

/* Anzeigewerte für Einsatzliste und Einsatzseite (SUL-119). Kalendertage in Ortszeit. */

export const STATE_CHIP: Record<MissionState, { ton: StatusTon; wort: string }> = {
  running: { ton: 'normal', wort: 'Laufend' },
  prepared: { ton: 'achtung', wort: 'Vorbereitet' },
  closed: { ton: 'neutral', wort: 'Abgeschlossen' },
};

const pad = (n: number) => String(n).padStart(2, '0');

/** „01:12“ (Stunden:Minuten), für Kennzahlen mit Einheit „h“. */
export const hoursMinutes = (ms: number) => {
  const min = Math.max(0, Math.floor(ms / 60_000));
  return `${pad(Math.floor(min / 60))}:${pad(min % 60)}`;
};

/** Dauer des Einsatzes: abgeschlossen bis zum Abschluss, laufend bis jetzt, vorbereitet keine. */
export const missionDurationMs = (m: Mission, nowMs: number): number | null =>
  m.started_at_ms == null ? null : (m.closed_at_ms ?? nowMs) - m.started_at_ms;

/** Der Tag, nach dem die Liste ordnet: Beginn, sonst Anlage. */
export const missionDayMs = (m: Mission) => m.started_at_ms ?? m.created_at_ms;

export const dateText = (ms: number) =>
  new Date(ms).toLocaleDateString('de-DE', { day: '2-digit', month: '2-digit', year: 'numeric' });
export const timeText = (ms: number) =>
  new Date(ms).toLocaleTimeString('de-DE', { hour: '2-digit', minute: '2-digit' });
export const dateTimeText = (ms: number) => `${dateText(ms)} ${timeText(ms)}`;

/** Gruppe in der Liste: vorbereitete oben, das Archiv nach Monaten („November 2026“). */
export const listGroup = (m: Mission) =>
  m.state === 'prepared'
    ? 'Vorbereitet'
    : new Date(missionDayMs(m)).toLocaleDateString('de-DE', { month: 'long', year: 'numeric' });

/** Vorbereitete zuerst (neueste oben), dann nach Tag absteigend. */
export const listOrder = (a: Mission, b: Mission) =>
  Number(a.state !== 'prepared') - Number(b.state !== 'prepared') || missionDayMs(b) - missionDayMs(a);

/** Trifft die Suche Name oder Ort? Ohne Groß-/Kleinschreibung. */
export const matchesSearch = (m: Mission, query: string) => {
  const q = query.trim().toLocaleLowerCase('de-DE');
  return !q || `${m.name} ${m.location ?? ''}`.toLocaleLowerCase('de-DE').includes(q);
};
