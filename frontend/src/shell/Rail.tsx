import { NavLink } from 'react-router';
import type { Role } from '../contract/generated/Role';
import {
  IconKarte,
  IconKarteGefuellt,
  IconKlemmbrett,
  IconKlemmbrettGefuellt,
  IconMonitor,
  IconPapierflieger,
  IconZahnrad,
  IconZahnradGefuellt,
  type IconPaar,
} from '../icons';

const ENTRIES: { to: string; label: string; icon: IconPaar; roles: Role[] }[] = [
  {
    to: '/',
    label: 'Lage',
    icon: { umriss: IconKarte, gefuellt: IconKarteGefuellt },
    roles: ['admin', 'operator'],
  },
  {
    to: '/einsaetze',
    label: 'Einsätze',
    icon: { umriss: IconKlemmbrett, gefuellt: IconKlemmbrettGefuellt },
    roles: ['admin', 'operator'],
  },
  // Lifelines Satz hat für Flug und System keinen gefüllten Zwilling: aktiv zeigt dasselbe Icon.
  {
    to: '/flights',
    label: 'Flüge',
    icon: { umriss: IconPapierflieger, gefuellt: IconPapierflieger },
    roles: ['admin', 'operator'],
  },
  { to: '/system', label: 'System', icon: { umriss: IconMonitor, gefuellt: IconMonitor }, roles: ['admin'] },
  {
    to: '/settings',
    label: 'Einstellungen',
    icon: { umriss: IconZahnrad, gefuellt: IconZahnradGefuellt },
    roles: ['admin'],
  },
];

/** Bereichsleiste am Arbeitsplatz: Icon über dem Etikett wie in Lifeline, aktiv mit dem gefüllten Icon. */
export function Rail({ role }: { role: Role }) {
  return (
    <nav className="sl-rail" aria-label="Bereiche">
      {ENTRIES.filter((e) => e.roles.includes(role)).map((e) => (
        <NavLink key={e.to} to={e.to} end={e.to === '/'}>
          {({ isActive }) => {
            const Icon = isActive ? e.icon.gefuellt : e.icon.umriss;
            return (
              <>
                <Icon size={20} className="sl-rail-icon" />
                {e.label}
              </>
            );
          }}
        </NavLink>
      ))}
    </nav>
  );
}
