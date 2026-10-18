import { useSearchParams } from 'react-router';
import { useMe, useMissions } from '../api/queries';
import { resolveScope, type MissionScope } from './scope';

/** Gewählter Einsatz aus `?einsatz=`. Das Großbild folgt immer dem laufenden Einsatz. */
export function useMissionScope(): { scope: MissionScope; setScope: (param: string) => void } {
  const [params, setParams] = useSearchParams();
  const kiosk = useMe().data?.role === 'kiosk';
  const { data } = useMissions();
  const scope = resolveScope(kiosk ? null : params.get('einsatz'), data ?? []);
  const setScope = (param: string) =>
    setParams((prev) => {
      const next = new URLSearchParams(prev);
      next.set('einsatz', param);
      return next;
    });
  return { scope, setScope };
}
