import { useEffect, useState } from 'react';

/** Ortszeit im Kopf, sekundengenau. */
export function Clock() {
  const [now, setNow] = useState(() => new Date());
  useEffect(() => {
    const id = window.setInterval(() => setNow(new Date()), 1000);
    return () => window.clearInterval(id);
  }, []);
  return (
    <time className="sl-clock" dateTime={now.toISOString()}>
      {now.toLocaleTimeString('de-DE')}
    </time>
  );
}
