import { useQueryClient } from '@tanstack/react-query';
import { useEffect, useRef, useState } from 'react';
import { useNavigate } from 'react-router';
import { ApiError, apiSend } from '../api/client';
import { queryKeys } from '../api/queries';

const INCOMPLETE = 'Der Zugangslink ist unvollständig. Den Link aus Einstellungen → Großbild erneut öffnen.';

/** Koppelt ein Großbild über den Zugangslink `/kiosk#<token>`. Das Token steht im Fragment, damit es nie im Server-Protokoll landet. */
export default function KioskEnrollPage() {
  const [token] = useState(() => window.location.hash.slice(1));
  const [serverError, setServerError] = useState<string | null>(null);
  const started = useRef(false);
  const navigate = useNavigate();
  const client = useQueryClient();

  useEffect(() => {
    // StrictMode führt den Effekt doppelt aus; das Gerät soll sich nur einmal anmelden.
    if (!token || started.current) return;
    started.current = true;
    apiSend('POST', '/api/auth/kiosk', { token })
      .then(async () => {
        history.replaceState(null, '', '/kiosk'); // Token nicht in der Adresszeile stehen lassen
        await client.invalidateQueries({ queryKey: queryKeys.me });
        navigate('/', { replace: true });
      })
      .catch((e: unknown) => setServerError(e instanceof ApiError ? e.message : 'Server nicht erreichbar.'));
  }, [token, client, navigate]);

  const error = token ? serverError : INCOMPLETE;
  return (
    <div className="sl-center">
      <section className="sl-card">
        <h1>Großbild koppeln</h1>
        <p role={error ? 'alert' : 'status'}>{error ?? 'Gerät wird angemeldet …'}</p>
      </section>
    </div>
  );
}
