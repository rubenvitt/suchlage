import { NavLink } from 'react-router';

/** Untere Leiste statt Rail: mobil nur zuschauen, deshalb nur Lage und Flüge. */
export function MobileTabs() {
  return (
    <nav className="sl-tabs" aria-label="Bereiche">
      <NavLink to="/" end>
        Lage
      </NavLink>
      <NavLink to="/flights">Flüge</NavLink>
    </nav>
  );
}
