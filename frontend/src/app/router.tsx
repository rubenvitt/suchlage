import { createBrowserRouter, Link } from 'react-router';
import { SettingsPage } from '../admin/SettingsPage';
import { SetupPage } from '../admin/SetupPage';
import { SystemPage } from '../admin/SystemPage';
import KioskEnrollPage from '../auth/KioskEnrollPage';
import LoginPage from '../auth/LoginPage';
import { FlightDetailPage } from '../flights/FlightDetailPage';
import { FlightsPage } from '../flights/FlightsPage';
import { MissionsPage } from '../missions/MissionsPage';
import { Frame } from '../shell/Frame';
import { LagePage } from '../views/LagePage';
import { RequireRole } from './RequireRole';
import { RouteError } from './RouteError';
import { SessionLayout } from './SessionLayout';

/** Alle Routen. Spätere Tasks ersetzen nur die Seitendateien, nie diese Tabelle. */
export const routes = [
  {
    // Eine Fehlerseite für alle Routen; die unbekannte Adresse (`*`) liegt unter der Sitzung, im Rahmen.
    errorElement: <RouteError />,
    children: [
      { path: '/anmelden', element: <LoginPage /> },
      { path: '/kiosk', element: <KioskEnrollPage /> },
      { path: '/einrichtung', element: <SetupPage /> },
      {
        element: <SessionLayout />,
        children: [
          { path: '/', element: <LagePage /> },
          {
            path: '/einsaetze',
            element: (
              <RequireRole roles={['admin', 'operator']}>
                <MissionsPage />
              </RequireRole>
            ),
          },
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
          {
            path: '*',
            element: (
              <Frame>
                <div className="sl-page">
                  <p>Diese Seite gibt es nicht.</p>
                  <Link to="/">Zur Lage</Link>
                </div>
              </Frame>
            ),
          },
        ],
      },
    ],
  },
];

export const router = () => createBrowserRouter(routes);
