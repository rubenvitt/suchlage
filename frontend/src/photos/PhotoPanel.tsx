import { Button } from 'antd';
import { useState } from 'react';
import { useDeleteShot } from '../api/queries';
import { DeleteButton } from '../components/DeleteButton';
import { Segmentleiste } from '../components/instrument';
import type { PhotoShot } from '../contract/generated/PhotoShot';
import { LENS_LABEL, shotTitle } from './photos';
import './photos.css';

interface Props {
  shot: PhotoShot;
  /** Original und Download nur für Konten; das Großbild zeigt nur das Vorschaubild. */
  canOpenOriginal: boolean;
  /** „Foto löschen …“ nur für die Verwaltung (Flugdetail und Lage). */
  canDelete?: boolean;
  onClose: () => void;
}

/** Vorschaubild einer Auslösung mit Umschalter der Linsen (Wärme zuerst, wie der Server sortiert). */
export function PhotoPanel({ shot, canOpenOriginal, canDelete = false, onClose }: Props) {
  const [chosenId, setChosenId] = useState<number | null>(null);
  const remove = useDeleteShot();
  const photo = shot.photos.find((p) => p.id === chosenId) ?? shot.photos[0];
  if (!photo) return null;
  return (
    <section className="sl-photo-panel" aria-label={`Foto ${shotTitle(shot)}`}>
      <header className="sl-photo-head">
        <span>{shotTitle(shot)}</span>
        <Button size="small" type="text" aria-label="Foto schließen" onClick={onClose}>
          ×
        </Button>
      </header>
      {shot.photos.length > 1 && (
        <Segmentleiste
          beschriftung="Linse wählen"
          optionen={shot.photos.map((p) => ({ wert: p.id, label: LENS_LABEL[p.lens] }))}
          wert={photo.id}
          onWechsel={setChosenId}
        />
      )}
      <div className="sl-photo-frame">
        {photo.thumb_url ? (
          <img src={photo.thumb_url} alt={`${LENS_LABEL[photo.lens]}: ${photo.name}`} />
        ) : (
          <p className="sl-muted">Vorschaubild entsteht …</p>
        )}
      </div>
      {!shot.position && <p className="sl-muted">Ohne Aufnahmeort – nicht auf der Karte.</p>}
      {canOpenOriginal && (
        <p className="sl-photo-links">
          <a className="sl-panel-link" href={photo.original_url} target="_blank" rel="noreferrer">
            Original öffnen
          </a>
          <a className="sl-panel-link" href={`${photo.original_url}&download=1`} download>
            Herunterladen
          </a>
        </p>
      )}
      {canDelete && (
        <DeleteButton
          label="Foto löschen …"
          title={`Foto ${shotTitle(shot)} löschen?`}
          pending={remove.isPending}
          error={remove.error}
          onOpen={remove.reset}
          onConfirm={() => remove.mutateAsync(shot.id)}
        >
          {shot.photos.length > 1
            ? `Gelöscht werden alle ${shot.photos.length} Linsen dieser Auslösung, Originale und Vorschaubilder.`
            : 'Gelöscht werden Original und Vorschaubild.'}
        </DeleteButton>
      )}
    </section>
  );
}
