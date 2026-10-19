import { Segmentleiste } from '../components/instrument';
import type { Lens } from '../contract/generated/Lens';
import { LENS_LABEL } from './copy';

interface Props {
  lenses: Lens[];
  value: Lens | null;
  onChange: (lens: Lens) => void;
  /** Sperrt die Wahl und nennt den Grund – eine stumme Sperre sähe aus wie kaputt. */
  disabled?: string;
}

/** Nur mit umschaltbaren Linsen, unter denen die gemeldete steht; sonst schweigt Pilot 2 auf den Wechsel. */
export function canSwitchLens(lenses: Lens[], value: Lens | null): value is Lens {
  return value != null && lenses.includes(value);
}

export function LensPicker({ lenses, value, onChange, disabled }: Props) {
  if (!canSwitchLens(lenses, value)) return null;
  return (
    <Segmentleiste
      beschriftung="Linse wählen"
      optionen={lenses.map((l) => ({
        wert: l,
        label: LENS_LABEL[l],
        gesperrt: l === value ? undefined : disabled,
      }))}
      wert={value}
      onWechsel={onChange}
    />
  );
}
