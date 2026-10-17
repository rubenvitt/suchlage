const n0 = new Intl.NumberFormat('de-DE', { maximumFractionDigits: 0 });
const n1 = new Intl.NumberFormat('de-DE', { minimumFractionDigits: 1, maximumFractionDigits: 1 });
// `exceptZero`: ein auf 0,0 gerundetes Sinken ist kein „-0,0“.
const n1Signed = new Intl.NumberFormat('de-DE', {
  minimumFractionDigits: 1,
  maximumFractionDigits: 1,
  signDisplay: 'exceptZero',
});
export const DASH = '–';
const pad = (n: number) => String(n).padStart(2, '0');
type Maybe = number | null | undefined;

/** Anzeigeformate der Instrumente: deutsche Zahlen, ein Strich statt einer erfundenen Null. */
export const fmt = {
  meters: (v: Maybe) => (v == null ? DASH : n0.format(v)),
  speed: (v: Maybe) => (v == null ? DASH : n1.format(v)),
  signedSpeed: (v: Maybe) => (v == null ? DASH : n1Signed.format(v)),
  heading: (v: Maybe) => (v == null ? DASH : String(((Math.round(v) % 360) + 360) % 360).padStart(3, '0')),
  percent: (v: Maybe) => (v == null ? DASH : n0.format(v)),
  clock: (ms: number) => new Date(ms).toLocaleTimeString('de-DE'),
  duration: (s: number) => {
    const t = Math.max(0, Math.floor(s));
    const h = Math.floor(t / 3600);
    const ms = `${pad(Math.floor((t % 3600) / 60))}:${pad(t % 60)}`;
    return h > 0 ? `${h}:${ms}` : ms;
  },
  distance: (m: Maybe): [string, string] =>
    m == null ? [DASH, 'm'] : m >= 1000 ? [n1.format(m / 1000), 'km'] : [n0.format(m), 'm'],
};
