import { Outlet } from 'react-router';
import { RequireRole } from './RequireRole';

/** Layout-Route aller angemeldeten Seiten. Hier liegt (ab F2) der eine Live-Strom, damit er beim Seitenwechsel bleibt. */
export function SessionLayout() {
  return (
    <RequireRole roles={['admin', 'operator', 'kiosk']}>
      <Outlet />
    </RequireRole>
  );
}
