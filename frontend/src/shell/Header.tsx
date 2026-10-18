import type { ReactNode } from 'react';
import type { Me } from '../contract/generated/Me';
import { BrandMark } from '../brand/BrandMark';
import { Clock } from './Clock';
import { UserMenu } from './UserMenu';

/** `mission`: der Einsatz als eigene Zelle vor dem Status (Skizze „Lage laufend“). */
export function Header({ mission, status, me }: { mission?: ReactNode; status?: ReactNode; me?: Me | null }) {
  return (
    <header className="sl-head">
      <div className="sl-mark" aria-hidden>
        <BrandMark height={22} lineColor="var(--lfh-rahmen-text)" />
      </div>
      <div className="sl-word">Suchlage</div>
      {/* Ohne Inhalt keine leere Zelle: `MissionHeader` ist selbst die Zelle (`.sl-mission-status`). */}
      {mission}
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
