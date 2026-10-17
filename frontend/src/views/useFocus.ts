import { useMemo } from 'react';
import { useSearchParams } from 'react-router';
import { useLive } from '../live/LiveProvider';
import { focusOf } from './selectors';

const CHOSEN_PARAM = 'rc';

/**
 * Fokus der Lage-Seite. Die Wahl der Fernsteuerung steht in der Adresse (`?rc=<SN>`): Kopf und
 * Ansicht lesen dieselbe Wahl, und sie übersteht ein Neuladen.
 */
export function useFocus() {
  // Der ganze Zustand ist eine stabile Referenz; der Fokus wird nur bei einer Änderung neu gebaut.
  const state = useLive((s) => s);
  const [params, setParams] = useSearchParams();
  const chosen = params.get(CHOSEN_PARAM) ?? undefined;
  const focus = useMemo(() => focusOf(state, chosen), [state, chosen]);
  const choose = (sn: string) => setParams({ [CHOSEN_PARAM]: sn }, { replace: true });
  return { state, focus, choose };
}
