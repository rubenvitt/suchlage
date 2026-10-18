import type { Mission } from '../contract/generated/Mission';

export type MissionScope = { kind: 'mission'; mission: Mission } | { kind: 'none' };

/** Zahl eines bekannten Einsatzes → dieser; `ohne` → none; sonst der laufende, ohne laufenden none. */
export function resolveScope(param: string | null, missions: Mission[]): MissionScope {
  if (param === 'ohne') return { kind: 'none' };
  const picked =
    param !== null && /^\d+$/.test(param) ? missions.find((m) => m.id === Number(param)) : undefined;
  const mission = picked ?? missions.find((m) => m.state === 'running');
  return mission ? { kind: 'mission', mission } : { kind: 'none' };
}

/** Wert für den Abfrageparameter der Server-Routen. */
export const scopeQuery = (scope: MissionScope): string =>
  scope.kind === 'mission' ? String(scope.mission.id) : 'none';

export const inScope = (missionId: number | null, scope: MissionScope): boolean =>
  scope.kind === 'mission' ? missionId === scope.mission.id : missionId === null;
