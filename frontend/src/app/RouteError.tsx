import { Button } from 'antd';
import { useEffect } from 'react';

const RELOAD_AFTER_MS = 30_000;

/**
 * Fängt jeden Renderfehler der Routen statt des englischen Entwicklerbildschirms von React Router.
 * Lädt selbst neu: Das Großbild hat keine Bedienung und muss sich selbst fangen.
 */
export function RouteError() {
  useEffect(() => {
    const timer = setTimeout(() => location.reload(), RELOAD_AFTER_MS);
    return () => clearTimeout(timer);
  }, []);
  return (
    <div className="sl-center">
      <section className="sl-card" role="alert">
        <h1>Die Ansicht ist abgestürzt.</h1>
        <p>Neuer Versuch in {RELOAD_AFTER_MS / 1000} s.</p>
        <Button type="primary" size="large" onClick={() => location.reload()}>
          Neu laden
        </Button>
      </section>
    </div>
  );
}
