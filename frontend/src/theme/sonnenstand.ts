// Übernommen aus Lifeline Hub f649f2f9 (27.10.2026): src/wetter/quelle.rs, `sonnenhoehe_grad` und `sonne_ueber_horizont` (nach TypeScript übertragen)

/** Ein Ort in WGS84-Grad. */
export interface Ort {
  lat: number;
  lon: number;
}

/**
 * Sonnenhöhe, ab der Tag gilt: −0,833° (Refraktion und Sonnenradius), wie Auf- und
 * Untergang definiert sind.
 */
const HORIZONT_GRAD = -0.833;

const RAD = Math.PI / 180;

/** Rest in [0, n), auch für negative Werte (wie `rem_euclid`). */
const rest = (x: number, n: number) => ((x % n) + n) % n;

/**
 * Höhe der Sonne über dem Horizont in Grad, am Ort zur Zeit `zeitMs` (Unix-Millisekunden).
 * Näherung des Astronomical Almanac (auch NOAA), auf etwa 0,01° genau — für Tag oder Nacht
 * mehr als genug. Rein.
 */
export function sonnenhoeheGrad({ lat, lon }: Ort, zeitMs: number): number {
  // Tage seit J2000.0 (2000-01-01 12:00 UT).
  const n = zeitMs / 86_400_000 + 2_440_587.5 - 2_451_545.0;
  const mittlereLaenge = rest(280.46 + 0.985_647_4 * n, 360);
  const anomalie = rest(357.528 + 0.985_600_3 * n, 360) * RAD;
  const ekliptikLaenge = (mittlereLaenge + 1.915 * Math.sin(anomalie) + 0.02 * Math.sin(2 * anomalie)) * RAD;
  const schiefe = (23.439 - 0.000_000_4 * n) * RAD;
  const rektaszension = Math.atan2(Math.cos(schiefe) * Math.sin(ekliptikLaenge), Math.cos(ekliptikLaenge));
  const deklination = Math.asin(Math.sin(schiefe) * Math.sin(ekliptikLaenge));
  const sternzeit = rest(280.460_618_37 + 360.985_647_366_29 * n, 360);
  const stundenwinkel = (sternzeit + lon - rektaszension / RAD) * RAD;
  const breite = lat * RAD;
  return (
    Math.asin(
      Math.sin(breite) * Math.sin(deklination) + Math.cos(breite) * Math.cos(deklination) * Math.cos(stundenwinkel),
    ) / RAD
  );
}

/** Steht die Sonne am Ort zur Zeit über dem Horizont (Tag nach der Definition von Auf- und Untergang)? Rein. */
export function sonneUeberHorizont(ort: Ort, zeitMs: number): boolean {
  return sonnenhoeheGrad(ort, zeitMs) > HORIZONT_GRAD;
}
