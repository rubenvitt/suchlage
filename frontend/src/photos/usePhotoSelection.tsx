import { useState, type ReactNode } from 'react';
import type { PhotoShot } from '../contract/generated/PhotoShot';
import { PhotoPanel } from './PhotoPanel';

/**
 * Auswahl einer Auslösung auf der Karte: Was die Karte bekommt und das Vorschaupaneel darüber.
 * Verschwindet die Auslösung aus der Liste, schließt das Paneel von selbst.
 */
export function usePhotoSelection(
  photos: PhotoShot[],
  canOpenOriginal: boolean,
): {
  mapProps: { selectedPhotoId: string | null; onPhotoClick: (id: string) => void };
  overlay: ReactNode;
  select: (id: string | null) => void;
} {
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const shot = photos.find((p) => p.id === selectedId) ?? null;
  return {
    mapProps: { selectedPhotoId: shot?.id ?? null, onPhotoClick: setSelectedId },
    overlay: shot && (
      // Neuer Schlüssel je Auslösung: Die Linsenwahl beginnt wieder beim Wärmebild.
      <PhotoPanel key={shot.id} shot={shot} canOpenOriginal={canOpenOriginal} onClose={() => setSelectedId(null)} />
    ),
    select: setSelectedId,
  };
}
