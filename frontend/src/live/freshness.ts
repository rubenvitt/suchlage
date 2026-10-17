import type { FreshnessThresholds } from '../contract/generated/FreshnessThresholds';

export type Freshness = 'fresh' | 'delayed' | 'lost';

/** Frische eines Werts gegen die Serveruhr, mit den Schwellen aus dem Snapshot. */
export function freshnessOf(lastSeenMs: number, serverNowMs: number, t: FreshnessThresholds): Freshness {
  const age = serverNowMs - lastSeenMs;
  if (age > t.lost_after_ms) return 'lost';
  if (age > t.delayed_after_ms) return 'delayed';
  return 'fresh';
}

/** „vor 9 s“, „vor 2 min“, „vor 2 h“ – nie „vor 0 s“, denn ein angezeigter Wert ist nie alterslos. */
export function ageLabel(ms: number): string {
  const s = Math.max(1, Math.round(ms / 1000));
  if (s < 60) return `vor ${s} s`;
  const min = Math.round(s / 60);
  if (min < 60) return `vor ${min} min`;
  return `vor ${Math.round(min / 60)} h`;
}
