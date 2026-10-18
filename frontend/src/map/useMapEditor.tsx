import { useMutation } from '@tanstack/react-query';
import { Button, Popconfirm } from 'antd';
import { useEffect, useEffectEvent, useState, type ReactNode } from 'react';
import { ApiError, apiSend } from '../api/client';
import type { GeoPoint } from '../contract/generated/GeoPoint';
import type { MapElement } from '../contract/generated/MapElement';
import type { MapElementInput } from '../contract/generated/MapElementInput';
import type { MapElementKind } from '../contract/generated/MapElementKind';
import type { MapElementUpdate } from '../contract/generated/MapElementUpdate';
import { useLiveDispatch } from '../live/LiveProvider';
import { DEFAULT_ELEMENT_COLOR } from './elementColors';
import {
  KIND_LABEL,
  addVertex,
  canFinish,
  defaultName,
  draftHint,
  removeLastVertex,
  startDraft,
  type Draft,
} from './elements';
import { ElementForm } from './ElementForm';

const TOOLS: MapElementKind[] = ['point', 'line', 'polygon'];

const errorText = (e: Error | null) =>
  e ? (e instanceof ApiError ? e.message : 'Server nicht erreichbar.') : null;

const time = (ms: number) => new Date(ms).toLocaleTimeString('de-DE', { hour: '2-digit', minute: '2-digit' });

function origin(e: MapElement) {
  const from = e.origin === 'pilot' ? `aus Pilot 2 (${e.author})` : e.author;
  return `${KIND_LABEL[e.kind]} · ${from} · ${time(e.updated_at_ms)}`;
}

/** Was die Karte vom Editor bekommt. */
export interface EditorMapProps {
  selectedElementId: string | null;
  draft: Draft | null;
  onMapClick?: (point: GeoPoint, elementId: string | null) => void;
}

/**
 * Zeichnen, Auswählen, Umbenennen, Umfärben und Löschen von Kartenelementen. Ohne `enabled` (keine
 * Rolle zum Ändern) bleibt die Karte ohne Bedienung. `onStartDrawing` schaltet etwa das Folgen ab:
 * Eine Karte, die mitläuft, lässt keine Eckpunkte setzen.
 */
export function useMapEditor(
  elements: MapElement[],
  { enabled, onStartDrawing }: { enabled: boolean; onStartDrawing?: () => void },
): { mapProps: EditorMapProps; overlay: ReactNode } {
  const [draft, setDraft] = useState<Draft | null>(null);
  const [pending, setPending] = useState<Draft | null>(null);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const dispatch = useLiveDispatch();
  const selected = elements.find((e) => e.id === selectedId) ?? null;

  const create = useMutation({
    mutationFn: (body: MapElementInput) => apiSend<MapElement>('POST', '/api/map/elements', body),
    onSuccess: (element) => {
      dispatch({ type: 'map_element', element });
      setPending(null);
      setSelectedId(element.id);
    },
  });
  const save = useMutation({
    mutationFn: ({ id, body }: { id: string; body: MapElementUpdate }) =>
      apiSend<MapElement>('PUT', `/api/map/elements/${encodeURIComponent(id)}`, body),
    onSuccess: (element) => dispatch({ type: 'map_element', element }),
  });
  const remove = useMutation({
    mutationFn: (id: string) => apiSend<void>('DELETE', `/api/map/elements/${encodeURIComponent(id)}`),
    onSuccess: (_, id) => {
      dispatch({ type: 'map_element_deleted', id });
      setSelectedId(null);
    },
  });

  const begin = (kind: MapElementKind) => {
    onStartDrawing?.();
    create.reset();
    setPending(null);
    setSelectedId(null);
    setDraft(startDraft(kind));
  };
  const finish = (d: Draft | null) => {
    if (!d || !canFinish(d)) return;
    setDraft(null);
    setPending(d);
  };
  const cancel = () => setDraft(null);

  // Escape bricht ab, Eingabe schließt ab, Rücktaste nimmt den letzten Eckpunkt zurück.
  const onKey = useEffectEvent((e: KeyboardEvent) => {
    if (!draft || e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) return;
    if (e.key === 'Escape') setDraft(null);
    else if (e.key === 'Enter') finish(draft);
    else if (e.key === 'Backspace') setDraft(removeLastVertex(draft));
    else return;
    e.preventDefault();
  });
  const drafting = draft != null;
  useEffect(() => {
    if (!drafting) return;
    const listener = (e: KeyboardEvent) => onKey(e);
    window.addEventListener('keydown', listener);
    return () => window.removeEventListener('keydown', listener);
  }, [drafting]);

  const onMapClick = (point: GeoPoint, elementId: string | null) => {
    if (draft) {
      const next = addVertex(draft, point);
      // Ein Punkt ist mit dem ersten Klick fertig.
      if (next.kind === 'point') finish(next);
      else setDraft(next);
      return;
    }
    if (pending) return;
    save.reset();
    remove.reset();
    setSelectedId(elementId);
  };

  if (!enabled) return { mapProps: { selectedElementId: null, draft: null }, overlay: null };

  let panel: ReactNode = null;
  if (pending) {
    panel = (
      <ElementForm
        key="neu"
        titel={`Neu: ${KIND_LABEL[pending.kind]}`}
        initialName={defaultName(pending.kind, elements)}
        initialColor={DEFAULT_ELEMENT_COLOR}
        submitLabel="Anlegen"
        cancelLabel="Verwerfen"
        busy={create.isPending}
        error={errorText(create.error)}
        onSubmit={(name, color) => create.mutate({ name, kind: pending.kind, points: pending.points, color, mission_id: null })}
        onCancel={() => setPending(null)}
      />
    );
  } else if (selected && !draft) {
    panel = (
      <ElementForm
        key={`${selected.id}-${selected.updated_at_ms}`}
        titel={selected.name}
        meta={origin(selected)}
        initialName={selected.name}
        initialColor={selected.color}
        submitLabel="Speichern"
        cancelLabel="Schließen"
        busy={save.isPending || remove.isPending}
        error={errorText(save.error ?? remove.error)}
        onSubmit={(name, color) => save.mutate({ id: selected.id, body: { name, color } })}
        onCancel={() => setSelectedId(null)}
        extra={
          <Popconfirm
            title={`„${selected.name}“ löschen?`}
            description="Das Element verschwindet auch auf der Fernsteuerung."
            okText="Löschen"
            okButtonProps={{ danger: true }}
            cancelText="Abbrechen"
            onConfirm={() => remove.mutate(selected.id)}
          >
            <Button danger disabled={remove.isPending} aria-label={`Element löschen: ${selected.name}`}>
              Löschen
            </Button>
          </Popconfirm>
        }
      />
    );
  }

  const bar = draft ? (
    <div className="sl-draw-bar" role="toolbar" aria-label={`${KIND_LABEL[draft.kind]} zeichnen`}>
      <span className="sl-draw-hint" aria-live="polite">
        <b>{KIND_LABEL[draft.kind]} zeichnen.</b> {draftHint(draft)}
      </span>
      {draft.kind !== 'point' && (
        <>
          <Button
            size="small"
            disabled={draft.points.length === 0}
            onClick={() => setDraft(removeLastVertex(draft))}
          >
            Letzten entfernen
          </Button>
          <Button size="small" type="primary" disabled={!canFinish(draft)} onClick={() => finish(draft)}>
            Fertig
          </Button>
        </>
      )}
      <Button size="small" onClick={cancel}>
        Abbrechen
      </Button>
    </div>
  ) : pending ? null : (
    <div className="sl-draw-bar" role="toolbar" aria-label="Zeichnen">
      <span className="sl-draw-hint">Zeichnen</span>
      {TOOLS.map((kind) => (
        <Button key={kind} size="small" onClick={() => begin(kind)}>
          {KIND_LABEL[kind]}
        </Button>
      ))}
    </div>
  );

  return {
    mapProps: { selectedElementId: selectedId, draft, onMapClick },
    overlay: (
      <div className="sl-map-editor">
        {panel}
        {bar}
      </div>
    ),
  };
}
