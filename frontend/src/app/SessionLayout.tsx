import { Outlet } from 'react-router';
import { LiveProvider } from '../live/LiveProvider';
import { RequireRole } from './RequireRole';

/**
 * Layout-Route aller angemeldeten Seiten. Hier liegt der eine Live-Strom: Er bleibt beim
 * Seitenwechsel bestehen, Lage und System teilen ihn, die Flugliste erneuert sich an ihm.
 */
export function SessionLayout() {
  return (
    <RequireRole roles={['admin', 'operator', 'kiosk']}>
      <LiveProvider>
        <Outlet />
      </LiveProvider>
    </RequireRole>
  );
}
