import { useEffect, useState } from 'react';
import { useMe } from '../api/queries';
import type { Role } from '../contract/generated/Role';

export type ViewMode = 'workstation' | 'kiosk' | 'mobile';
/** antds Schwelle `md`, dieselbe wie `istSchmal` in `components/useViewport.ts`. */
export const MOBILE_BELOW_PX = 768;

export function viewModeFor(role: Role, width: number): ViewMode {
  if (role === 'kiosk') return 'kiosk';
  return width < MOBILE_BELOW_PX ? 'mobile' : 'workstation';
}

export function useViewMode(): ViewMode {
  const { data: me } = useMe();
  const [width, setWidth] = useState(() => window.innerWidth);
  useEffect(() => {
    const onResize = () => setWidth(window.innerWidth);
    window.addEventListener('resize', onResize);
    return () => window.removeEventListener('resize', onResize);
  }, []);
  return viewModeFor(me?.role ?? 'operator', width);
}
