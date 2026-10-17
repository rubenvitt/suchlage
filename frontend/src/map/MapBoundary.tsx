import { GPUInitializationError } from 'maplibre-gl';
import { Component, type ReactNode } from 'react';

export const MAP_UNAVAILABLE_TITLE = 'Karte nicht verfügbar';
export const MAP_UNAVAILABLE_TEXT =
  'Dieser Browser bietet kein WebGL 2, etwa weil die Hardwarebeschleunigung aus ist. Die übrigen Anzeigen laufen weiter.';

interface Props {
  label: string;
  children: ReactNode;
}

/**
 * Fängt den fehlenden WebGL-2-Kontext der Karte ab, damit er nicht über die Fehlergrenze des Routers
 * die ganze Seite samt Livebild und Instrumenten ersetzt. Jeder andere Fehler ist einer im Code und
 * geht weiter nach oben.
 */
export class MapBoundary extends Component<Props, { error: unknown }> {
  state = { error: null as unknown };

  static getDerivedStateFromError(error: unknown) {
    return { error };
  }

  render() {
    const { error } = this.state;
    if (error === null) return this.props.children;
    if (!(error instanceof GPUInitializationError)) throw error;
    return (
      <div className="sl-map" role="region" aria-label={this.props.label}>
        <div className="sl-map-unavailable" role="alert">
          <b>{MAP_UNAVAILABLE_TITLE}</b>
          <span>{MAP_UNAVAILABLE_TEXT}</span>
        </div>
      </div>
    );
  }
}
