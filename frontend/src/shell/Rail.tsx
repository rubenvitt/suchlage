import { NavLink } from 'react-router';
import type { Role } from '../contract/generated/Role';

const ENTRIES: { to: string; label: string; roles: Role[] }[] = [
  { to: '/', label: 'Lage', roles: ['admin', 'operator'] },
  { to: '/einsaetze', label: 'Einsätze', roles: ['admin', 'operator'] },
  { to: '/flights', label: 'Flüge', roles: ['admin', 'operator'] },
  { to: '/system', label: 'System', roles: ['admin'] },
  { to: '/settings', label: 'Einstellungen', roles: ['admin'] },
];

/** Bereichsleiste am Arbeitsplatz. Nur Beschriftungen: Symbole kommen erst, wenn die Icon-Lizenz geklärt ist. */
export function Rail({ role }: { role: Role }) {
  return (
    <nav className="sl-rail" aria-label="Bereiche">
      {ENTRIES.filter((e) => e.roles.includes(role)).map((e) => (
        <NavLink key={e.to} to={e.to} end={e.to === '/'}>
          {e.label}
        </NavLink>
      ))}
    </nav>
  );
}
