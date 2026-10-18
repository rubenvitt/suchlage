import { useMemo } from 'react';
import { useSearchParams } from 'react-router';
import { useMe, useMissions } from '../api/queries';
import { resolveScope, type MissionScope } from './scope';

/**
 * Gewählter Einsatz aus `?einsatz=`. Das Großbild folgt immer dem laufenden Einsatz. `ready`: Die
 * Einsätze sind bekannt; vorher wäre der Scope stillschweigend „ohne Einsatz“. Ein gescheiterter
 * Neuabruf behält die alten Daten und bleibt `ready`.
 */
export function useMissionScope(): {
  scope: MissionScope;
  setScope: (param: string) => void;
  ready: boolean;
} {
  const [params, setParams] = useSearchParams();
  const kiosk = useMe().data?.role === 'kiosk';
  const { data } = useMissions();
  const param = kiosk ? null : params.get('einsatz');
  // Stabile Referenz: Die Kartendaten hängen am Scope und sollen nicht jede Sekunde neu entstehen.
  const scope = useMemo(() => resolveScope(param, data ?? []), [param, data]);
  const setScope = (param: string) =>
    setParams((prev) => {
      const next = new URLSearchParams(prev);
      next.set('einsatz', param);
      return next;
    });
  return { scope, setScope, ready: data !== undefined };
}
