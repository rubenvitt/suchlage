import { Button } from 'antd';
import type { ReactNode } from 'react';
import { Link, Navigate, useLocation } from 'react-router';
import { useMe } from '../api/queries';
import type { Role } from '../contract/generated/Role';

/** Schranke für angemeldete Seiten: ohne Sitzung zur Anmeldung, mit falscher Rolle ein Hinweis. */
export function RequireRole({ roles, children }: { roles: Role[]; children: ReactNode }) {
  const { data: me, isPending, isError, isFetching, refetch } = useMe();
  const location = useLocation();
  if (isPending) {
    return (
      <p className="sl-center" aria-busy="true">
        Anmeldung wird geprüft …
      </p>
    );
  }
  if (isError) {
    return (
      <div className="sl-center">
        <section className="sl-card" role="alert">
          <p>Server nicht erreichbar. Ohne Server lässt sich die Anmeldung nicht prüfen.</p>
          <Button loading={isFetching} onClick={() => void refetch()}>
            Erneut versuchen
          </Button>
        </section>
      </div>
    );
  }
  if (!me)
    return (
      <Navigate to={`/anmelden?next=${encodeURIComponent(location.pathname + location.search)}`} replace />
    );
  if (!roles.includes(me.role)) {
    return (
      <div className="sl-center">
        <section className="sl-card" role="alert">
          <p>
            {me.role === 'kiosk'
              ? 'Das Großbild zeigt nur die Lage.'
              : 'Diese Seite ist der Verwaltung vorbehalten.'}
          </p>
          <Link to="/">Zur Lage</Link>
        </section>
      </div>
    );
  }
  return <>{children}</>;
}
