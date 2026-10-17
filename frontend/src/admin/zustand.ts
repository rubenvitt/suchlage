import type { PaneelDatenzustand } from '../components/instrument';

/**
 * Die eine Abbildung einer Abfrage auf die vier Paneelzustände. Strukturell statt `UseQueryResult<…>`:
 * passt auf jede Abfrage. Scheitert nur ein Hintergrund-Abruf, bleiben die Daten stehen (wie in F1);
 * der Fehlerzweig gilt erst, wenn es gar keine Daten gibt.
 */
export function zustandOf(q: { isPending: boolean; isError: boolean; data?: unknown }): PaneelDatenzustand {
  if (q.isPending) return 'laden';
  if (q.isError && q.data === undefined) return 'fehler';
  return Array.isArray(q.data) && q.data.length === 0 ? 'leer' : 'daten';
}
