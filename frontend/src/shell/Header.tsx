import type { ReactNode } from 'react';
import type { Me } from '../contract/generated/Me';
import { Clock } from './Clock';
import { UserMenu } from './UserMenu';

/** `mission`: der Einsatz als eigene Zelle vor dem Status (Skizze „Lage laufend“). */
export function Header({ mission, status, me }: { mission?: ReactNode; status?: ReactNode; me?: Me | null }) {
  return (
    <header className="sl-head">
      <div className="sl-mark" aria-hidden />
      <div className="sl-word">Suchlage</div>
      {mission && <div className="sl-mission-cell">{mission}</div>}
      <div className="sl-status">{status}</div>
      <div>
        <Clock />
      </div>
      {me && (
        <div>
          <UserMenu me={me} />
        </div>
      )}
    </header>
  );
}
