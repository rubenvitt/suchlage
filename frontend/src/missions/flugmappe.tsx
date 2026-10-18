import { StatusChip } from '../components/instrument';
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

/** „mit Flugmappe verknüpft · Name · 14.10., 08:46“; `null` ohne Verknüpfung. */
export function linkText(m: Mission): string | null {
  return m.flugmappe
    ? `mit Flugmappe verknüpft · ${m.flugmappe.linked_by} · ${at(m.flugmappe.linked_at_ms)}`
    : null;
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
