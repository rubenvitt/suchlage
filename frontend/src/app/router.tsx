import { createBrowserRouter } from 'react-router';
import { SettingsPage } from '../admin/SettingsPage';
import { SetupPage } from '../admin/SetupPage';
import { SystemPage } from '../admin/SystemPage';
import KioskEnrollPage from '../auth/KioskEnrollPage';
import LoginPage from '../auth/LoginPage';
import { FlightDetailPage } from '../flights/FlightDetailPage';
import { FlightsPage } from '../flights/FlightsPage';
import { LagePage } from '../views/LagePage';
import { RequireRole } from './RequireRole';
import { SessionLayout } from './SessionLayout';

/** Alle Routen. Spätere Tasks ersetzen nur die Seitendateien, nie diese Tabelle. */
export const routes = [
  { path: '/anmelden', element: <LoginPage /> },
  { path: '/kiosk', element: <KioskEnrollPage /> },
  { path: '/einrichtung', element: <SetupPage /> },
  {
    element: <SessionLayout />,
    children: [
      { path: '/', element: <LagePage /> },
      {
        path: '/flights',
        element: (
          <RequireRole roles={['admin', 'operator']}>
            <FlightsPage />
          </RequireRole>
        ),
      },
      {
        path: '/flights/:id',
        element: (
          <RequireRole roles={['admin', 'operator']}>
            <FlightDetailPage />
          </RequireRole>
        ),
      },
      {
        path: '/settings',
        element: (
          <RequireRole roles={['admin']}>
            <SettingsPage />
          </RequireRole>
        ),
      },
      {
        path: '/system',
        element: (
          <RequireRole roles={['admin']}>
            <SystemPage />
          </RequireRole>
        ),
      },
    ],
  },
];

export const router = () => createBrowserRouter(routes);
