import type { ReactNode } from 'react';
import { useMe } from '../api/queries';
import { useViewMode } from '../app/viewMode';
import { Header } from './Header';
import { MobileTabs } from './MobileTabs';
import { Rail } from './Rail';

/** Rahmen jeder angemeldeten Seite: Kopf, je nach Ansicht Rail oder untere Leiste. Das Großbild hat keine Bedienung. */
export function Frame({ status, children }: { status?: ReactNode; children: ReactNode }) {
  const { data: me } = useMe();
  const mode = useViewMode();
  return (
    <div className="sl-frame" data-mode={mode}>
      <Header status={status} me={mode === 'kiosk' ? undefined : me} />
      {mode === 'workstation' && me && <Rail role={me.role} />}
      <main className="sl-main">{children}</main>
      {mode === 'mobile' && <MobileTabs />}
    </div>
  );
}
