import { Button } from 'antd';
import { StatusChip, type StatusTon } from '../components/instrument';
import type { FlugmappeLink } from '../contract/generated/FlugmappeLink';
import type { Mission } from '../contract/generated/Mission';
import type { MissionSection } from '../contract/generated/MissionSection';

/** Erklärung, wo bei verknüpften Einsätzen „Abschnitt anlegen“ stünde (Server: 422). */
export const SECTIONS_FROM_FLUGMAPPE = 'Abschnitte kommen aus der Flugmappe';

const at = (ms: number) =>
  new Date(ms).toLocaleString('de-DE', {
    day: '2-digit',
    month: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
  });

/** Nicht gelöste Verknüpfungen: Diese Flugmappen arbeiten (noch) am Einsatz. */
export const activeLinks = (m: Mission) => m.flugmappe_links.filter((l) => l.released_at_ms == null);

/**
 * „mit Flugmappe verknüpft · Matrice 350 RTK · Name · 14.10., 08:46“, bei mehreren „mit 2 Flugmappen
 * verknüpft · Matrice 350 RTK, Matrice 200“; `null` ohne nicht gelöste Verknüpfung.
 */
export function linkText(m: Mission): string | null {
  const links = activeLinks(m);
  if (links.length === 0) return null;
  if (links.length === 1) {
    const [l] = links;
    return `mit Flugmappe verknüpft · ${l.aircraft_label} · ${l.linked_by} · ${at(l.linked_at_ms)}`;
  }
  return `mit ${links.length} Flugmappen verknüpft · ${links.map((l) => l.aircraft_label).join(', ')}`;
}

/** Herkunft und Verknüpfung in einer Zeile (Kopfleiste); `null` bei reinen Suchlage-Einsätzen. */
export function flugmappeHint(m: Mission): string | null {
  const parts = [m.origin === 'flugmappe' ? 'aus der Flugmappe' : null, linkText(m)].filter(Boolean);
  return parts.length ? parts.join(' · ') : null;
}

/** Hinweis unter dem Einsatz in Listen: Herkunft als Chip, Verknüpfung als Text. */
export function FlugmappeNote({ mission }: { mission: Mission }) {
  const link = linkText(mission);
  if (mission.origin !== 'flugmappe' && !link) return null;
  return (
    <p className="sl-mission-fm">
      {mission.origin === 'flugmappe' && <StatusChip ton="neutral" wort="aus der Flugmappe" />}
      {link && <span>{link}</span>}
    </p>
  );
}

/** Zustand einer Verknüpfung: offen (arbeitet), fertig (Teil abgeschlossen), gelöst (in Suchlage). */
export function linkState(l: FlugmappeLink): { wort: string; ton: StatusTon } {
  if (l.released_at_ms != null) return { wort: 'gelöst', ton: 'neutral' };
  if (l.done_at_ms != null) return { wort: 'fertig', ton: 'normal' };
  return { wort: 'offen', ton: 'bedien' };
}

/**
 * Verknüpfte Flugmappen eines Einsatzes mit Fluggerät, wer verknüpft hat, und Zustand. Mit
 * `onRelease` je nicht gelöster ein „Lösen“ (verlorenes Gerät).
 */
export function FlugmappeLinks({
  mission,
  onRelease,
  releasing,
}: {
  mission: Mission;
  onRelease?: (link: FlugmappeLink) => void;
  /** Die Verknüpfung, die gerade gelöst wird. */
  releasing?: number | null;
}) {
  if (mission.flugmappe_links.length === 0) return null;
  return (
    <ul className="sl-fm-links" aria-label={`Flugmappen von ${mission.name}`}>
      {mission.flugmappe_links.map((l) => {
        const state = linkState(l);
        return (
          <li key={l.id}>
            <strong>{l.aircraft_label}</strong>
            <span>
              {l.linked_by} · {at(l.linked_at_ms)}
            </span>
            <StatusChip ton={state.ton} wort={state.wort} />
            {onRelease && l.released_at_ms == null && (
              <Button
                size="small"
                loading={releasing === l.id}
                aria-label={`Lösen: Flugmappe ${l.aircraft_label}`}
                onClick={() => onRelease(l)}
              >
                Lösen
              </Button>
            )}
          </li>
        );
      })}
    </ul>
  );
}

/** Kartenfoto eines Abschnitts; `map_photo_at_ms` als Versionszeichen, damit ein neues Foto nicht aus dem Cache kommt. */
export const mapPhotoUrl = (missionId: number, s: MissionSection) =>
  `/api/missions/${missionId}/sections/${s.id}/map-photo?v=${s.map_photo_at_ms ?? 0}`;

/** Vorschaubild des Kartenfotos; der Klick öffnet es in voller Größe in einem neuen Tab. */
export function MapPhotoThumb({ missionId, section }: { missionId: number; section: MissionSection }) {
  if (!section.has_map_photo) return null;
  const url = mapPhotoUrl(missionId, section);
  return (
    <a
      className="sl-map-photo"
      href={url}
      target="_blank"
      rel="noreferrer"
      aria-label={`Kartenfoto ${section.label} in voller Größe öffnen`}
    >
      <img src={url} alt={`Kartenfoto ${section.label}`} loading="lazy" />
    </a>
  );
}

/** Abschnitte eines Einsatzes, älteste zuerst, je mit Kartenfoto, wenn eines vorliegt. */
export function SectionList({ mission }: { mission: Mission }) {
  if (mission.sections.length === 0) return null;
  return (
    <ul className="sl-mission-sections" aria-label={`Abschnitte von ${mission.name}`}>
      {mission.sections.map((s) => (
        <li key={s.id}>
          <span>{s.label}</span>
          <MapPhotoThumb missionId={mission.id} section={s} />
        </li>
      ))}
    </ul>
  );
}
