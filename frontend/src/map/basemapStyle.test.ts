import { basemapStyle } from './basemapStyle';

// Ein falscher Fontstack fällt nur draußen auf: Die Karte zeichnet, die Beschriftung fehlt still.
test.each(['dark', 'light'] as const)('%s: Beschriftung nutzt den Fontstack von VersaTiles', (theme) => {
  const style = basemapStyle(theme);
  expect(style.glyphs).toBe('https://tiles.versatiles.org/assets/glyphs/{fontstack}/{range}.pbf');
  const fonts = style.layers.flatMap((l) => {
    const font = (l.layout as { 'text-font'?: string[] } | undefined)?.['text-font'];
    return font ?? [];
  });
  expect(fonts.length).toBeGreaterThan(0);
  expect(new Set(fonts)).toEqual(new Set(['noto_sans_regular']));
});

test('Hell und Dunkel unterscheiden sich, Quelle und Namen bleiben gleich', () => {
  const dark = basemapStyle('dark');
  const light = basemapStyle('light');
  expect(dark.layers.map((l) => l.id)).toEqual(light.layers.map((l) => l.id));
  expect(dark.layers[0]).not.toEqual(light.layers[0]);
  expect(dark.sources).toEqual(light.sources);
  expect(new Set(dark.layers.map((l) => l.id)).size).toBe(dark.layers.length);
});
