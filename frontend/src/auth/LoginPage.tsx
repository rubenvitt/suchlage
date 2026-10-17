import { useQueryClient } from '@tanstack/react-query';
import { Alert, Button } from 'antd';
import { useState } from 'react';
import { useNavigate, useSearchParams } from 'react-router';
import { ApiError, apiSend } from '../api/client';
import { queryKeys, useAuthOptions } from '../api/queries';

/** Nur Pfade dieser Seite als Rücksprung; `//host` oder `https://…` aus einem fremden Link führen zur Lage. */
function sameOriginPath(next: string | null): string {
  return next?.startsWith('/') && !next.startsWith('//') && !next.startsWith('/\\') ? next : '/';
}

export default function LoginPage() {
  const [params] = useSearchParams();
  const next = sameOriginPath(params.get('next'));
  const { data: options, isPending, isError, isFetching, refetch } = useAuthOptions();
  const navigate = useNavigate();
  const client = useQueryClient();
  const [devError, setDevError] = useState<string | null>(null);

  async function devLogin() {
    setDevError(null);
    try {
      await apiSend('POST', '/api/auth/dev-login');
    } catch (e) {
      setDevError(e instanceof ApiError ? e.message : 'Server nicht erreichbar.');
      return;
    }
    // Auch ohne Beobachter neu holen: Die Zielseite sähe sonst das alte `null` und leitete zurück zur Anmeldung.
    await client.invalidateQueries({ queryKey: queryKeys.me, refetchType: 'all' });
    navigate(next, { replace: true });
  }

  return (
    <div className="sl-center">
      <section className="sl-card" aria-labelledby="login-title">
        <h1 id="login-title">Suchlage</h1>
        <p>Livebild, Position und Flugspur der Drohne für die Einsatzleitung.</p>
        {params.get('fehler') === 'oidc' && (
          <Alert type="error" title="Anmeldung abgebrochen. Bitte erneut anmelden." />
        )}
        {isPending && <p aria-busy="true">Anmeldeverfahren werden abgerufen …</p>}
        {isError && (
          <>
            <Alert type="error" title="Server nicht erreichbar. Ohne Server ist keine Anmeldung möglich." />
            <Button loading={isFetching} onClick={() => void refetch()}>
              Erneut versuchen
            </Button>
          </>
        )}
        {options?.setup_required && (
          <p>
            Suchlage ist noch nicht eingerichtet. Den Einrichtungslink aus dem Server-Protokoll öffnen – er
            gilt 24 Stunden.
          </p>
        )}
        {options?.oidc && (
          <Button type="primary" size="large" href={`/api/auth/oidc/start?next=${encodeURIComponent(next)}`}>
            Mit Pocket ID anmelden
          </Button>
        )}
        {options?.dev_login && (
          <Button size="large" onClick={() => void devLogin()}>
            Als Entwicklung anmelden
          </Button>
        )}
        {devError && <Alert type="error" title={devError} />}
      </section>
    </div>
  );
}
