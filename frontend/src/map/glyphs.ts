/** Entwurfsraster der Glyphen; gezeichnet wird in `size` Pixeln, skaliert aus diesem Raster. */
const AIRCRAFT_GRID = 40;
const HOME_GRID = 24;

function canvas(size: number, grid: number) {
  const c = document.createElement('canvas');
  c.width = c.height = size;
  const g = c.getContext('2d')!;
  g.scale(size / grid, size / grid);
  return g;
}

/**
 * Kurs-Glyphe: spitzes Dreieck, nach oben = Norden, MapLibre dreht per `icon-rotate`.
 * `size` in Gerätepixeln; für HiDPI `addImage(…, { pixelRatio })` mit passendem Vielfachen.
 */
export function aircraftGlyph(color: string, outlineOnly: boolean, size = AIRCRAFT_GRID): ImageData {
  const g = canvas(size, AIRCRAFT_GRID);
  g.beginPath();
  g.moveTo(20, 3);
  g.lineTo(33, 35);
  g.lineTo(20, 27);
  g.lineTo(7, 35);
  g.closePath();
  g.lineWidth = 3;
  g.lineJoin = 'round';
  g.strokeStyle = color;
  g.stroke();
  if (!outlineOnly) {
    g.fillStyle = color;
    g.fill();
  }
  return g.getImageData(0, 0, size, size);
}

/** Startpunkt: ⌂ als Umriss, damit er unter Spur und Fluggerät zurücktritt. */
export function homeGlyph(color: string, size = HOME_GRID): ImageData {
  const g = canvas(size, HOME_GRID);
  g.strokeStyle = color;
  g.lineWidth = 2;
  g.lineJoin = 'round';
  g.beginPath();
  g.moveTo(4, 12);
  g.lineTo(12, 4);
  g.lineTo(20, 12);
  g.moveTo(6, 10);
  g.lineTo(6, 20);
  g.lineTo(18, 20);
  g.lineTo(18, 10);
  g.stroke();
  return g.getImageData(0, 0, size, size);
}
