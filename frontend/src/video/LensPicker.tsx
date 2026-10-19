import { Segmentleiste } from '../components/instrument';
import type { Lens } from '../contract/generated/Lens';
import { LENS_LABEL } from './copy';

/** Linsen der H20N, falls das Fluggerät keine umschaltbaren Linsen meldet. */
const FALLBACK_LENSES: Lens[] = ['wide', 'zoom', 'thermal'];

interface Props {
  lenses: Lens[];
  value: Lens | null;
  onChange: (lens: Lens) => void;
  /** Sperrt die Wahl und nennt den Grund – eine stumme Sperre sähe aus wie kaputt. */
  disabled?: string;
}

/**
 * Immer sichtbar, solange ein Livebild läuft: Meldet das Gerät keine umschaltbaren Linsen, stehen
 * die der H20N zur Wahl. Steht die gemeldete Linse nicht in der Liste (etwa „normal“), ist keine
 * als aktiv markiert, statt fälschlich die erste.
 */
export function LensPicker({ lenses, value, onChange, disabled }: Props) {
  const options = lenses.length > 0 ? lenses : FALLBACK_LENSES;
  const current = value && options.includes(value) ? value : null;
  return (
    <Segmentleiste
      beschriftung="Linse wählen"
      optionen={options.map((l) => ({
        wert: l,
        label: LENS_LABEL[l],
        gesperrt: l === current ? undefined : disabled,
      }))}
      wert={current}
      onWechsel={onChange}
    />
  );
}
