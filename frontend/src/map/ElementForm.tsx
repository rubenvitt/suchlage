import { Alert, Button, Input } from 'antd';
import { useId, useState, type ReactNode } from 'react';
import { Paneel } from '../components/instrument';
import { ELEMENT_COLORS } from './elementColors';

/** Wie die Bezeichnungen in der Verwaltung; der Server erlaubt bis 120 für Namen aus Pilot 2. */
export const NAME_MAX = 80;

interface Props {
  titel: ReactNode;
  meta?: ReactNode;
  initialName: string;
  initialColor: string;
  submitLabel: string;
  cancelLabel: string;
  busy: boolean;
  error: string | null;
  onSubmit: (name: string, color: string) => void;
  onCancel: () => void;
  /** Weitere Aktion neben den Knöpfen, etwa Löschen. */
  extra?: ReactNode;
}

/** Name und Farbe eines Kartenelements: beim Anlegen und für ein ausgewähltes Element. */
export function ElementForm({
  titel,
  meta,
  initialName,
  initialColor,
  submitLabel,
  cancelLabel,
  busy,
  error,
  onSubmit,
  onCancel,
  extra,
}: Props) {
  const [name, setName] = useState(initialName);
  const [color, setColor] = useState(initialColor.toUpperCase());
  const nameId = useId();
  const colorId = useId();
  const trimmed = name.trim();
  const submit = () => {
    if (trimmed) onSubmit(trimmed, color);
  };
  return (
    <Paneel titel={titel} meta={meta} ueberschrift="h3" koerperPolster className="sl-element-form">
      <form
        className="sl-element-fields"
        onSubmit={(e) => {
          e.preventDefault();
          submit();
        }}
      >
        <label htmlFor={nameId}>Name</label>
        <Input
          id={nameId}
          value={name}
          maxLength={NAME_MAX}
          onChange={(e) => setName(e.target.value)}
          status={trimmed ? undefined : 'error'}
        />
        <span id={colorId}>Farbe</span>
        <div role="radiogroup" aria-labelledby={colorId} className="sl-swatches">
          {ELEMENT_COLORS.map((c) => (
            <button
              key={c.wert}
              type="button"
              role="radio"
              aria-checked={color === c.wert}
              aria-label={c.label}
              title={c.label}
              className="sl-swatch"
              style={{ background: c.wert }}
              onClick={() => setColor(c.wert)}
            />
          ))}
        </div>
        {error && <Alert type="error" title={error} />}
        <div className="sl-element-actions">
          <Button type="primary" htmlType="submit" loading={busy} disabled={!trimmed}>
            {submitLabel}
          </Button>
          <Button onClick={onCancel} disabled={busy}>
            {cancelLabel}
          </Button>
          {extra}
        </div>
      </form>
    </Paneel>
  );
}
