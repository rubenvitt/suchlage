import { Button } from 'antd';
import { QRCodeSVG } from 'qrcode.react';
import { useState } from 'react';
import { farbenHell } from '../theme/tokens';
import './admin.css';

type CopyState = 'idle' | 'done' | 'failed';

const COPY_LABEL: Record<CopyState, string> = {
  idle: 'Adresse kopieren',
  done: 'Kopiert',
  failed: 'Kopieren nicht möglich',
};

/**
 * Ein Link, den der Server nur einmal herausgibt: Adresse, QR-Code und Kopierknopf. Der QR-Code steht
 * in jeder Darstellung dunkel auf hell mit Ruhezone; invertierte Codes lesen viele Scanner nicht.
 */
export function OnceVisibleLink({ url, hint }: { url: string; hint: string }) {
  const [copy, setCopy] = useState<CopyState>('idle');
  // Ohne HTTPS (Adresse im VPN) gibt es keine Zwischenablage; dann bleibt nur das Markieren.
  const copyUrl = () => {
    if (!navigator.clipboard) return setCopy('failed');
    navigator.clipboard.writeText(url).then(
      () => setCopy('done'),
      () => setCopy('failed'),
    );
  };
  return (
    <div className="sl-once" role="status">
      <QRCodeSVG
        value={url}
        size={160}
        marginSize={2}
        bgColor={farbenHell.flaeche}
        fgColor={farbenHell.text}
        role="img"
        aria-label="QR-Code der Adresse"
      />
      <div className="sl-once-body">
        <code className="sl-once-url">{url}</code>
        <p className="sl-muted">{hint} Die Adresse wird nur jetzt angezeigt.</p>
        <div className="sl-once-copy">
          <Button onClick={copyUrl}>{COPY_LABEL[copy]}</Button>
          {copy === 'failed' && <span className="sl-muted">Adresse markieren und von Hand kopieren.</span>}
        </div>
      </div>
    </div>
  );
}
