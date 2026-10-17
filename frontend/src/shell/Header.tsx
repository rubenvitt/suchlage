import type { ReactNode } from 'react';
import type { Me } from '../contract/generated/Me';
import { Clock } from './Clock';
import { UserMenu } from './UserMenu';

export function Header({ status, me }: { status?: ReactNode; me?: Me | null }) {
  return (
    <header className="sl-head">
      <div className="sl-mark" aria-hidden />
      <div className="sl-word">Suchlage</div>
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
