import type { ReactNode } from 'react';

/** Was gerade fehlt und was zu tun ist. Steht über der Karte, die Karte bleibt sichtbar. */
export function EmptyState({ title, text, action }: { title: string; text: string; action?: ReactNode }) {
  return (
    <div className="sl-empty" role="status">
      <h2>{title}</h2>
      <p>{text}</p>
      {action}
    </div>
  );
}
