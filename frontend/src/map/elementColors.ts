/**
 * Die sechs Farben, die Pilot 2 für Kartenelemente kennt (DJI-Tabelle in `10.create.md`). Die Doku
 * nennt `#B620E0` ORANGE und `#212121` PURPLE; hier heißen sie nach ihrem Aussehen. Gleiche Liste wie
 * `MAP_COLORS` im Vertrag (src/contract/map.rs).
 */
export const ELEMENT_COLORS = [
  { wert: '#2D8CF0', label: 'Blau' },
  { wert: '#19BE6B', label: 'Grün' },
  { wert: '#FFBB00', label: 'Gelb' },
  { wert: '#E23C39', label: 'Rot' },
  { wert: '#B620E0', label: 'Violett' },
  { wert: '#212121', label: 'Schwarz' },
] as const;

export const DEFAULT_ELEMENT_COLOR = ELEMENT_COLORS[3].wert;

export function colorLabel(wert: string): string {
  return ELEMENT_COLORS.find((c) => c.wert === wert.toUpperCase())?.label ?? wert;
}
