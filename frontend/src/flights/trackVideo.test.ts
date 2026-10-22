import { flightDetail } from '../contract/fixtures.generated';
import { nearestTrackTime, videoSecond } from './trackVideo';

const segments = flightDetail.segments;
const [first, second] = segments;

test('der nächste Spurpunkt gibt den Zeitpunkt, über Segmentgrenzen hinweg', () => {
  expect(nearestTrackTime(segments, { lat: 52.4302, lon: 10.0103 })).toBe(first![1]!.t_ms);
  // Knapp neben dem letzten Punkt des zweiten Segments.
  expect(nearestTrackTime(segments, { lat: 52.43212, lon: 10.01338 })).toBe(second![1]!.t_ms);
});

test('ohne Spur kein Zeitpunkt', () => {
  expect(nearestTrackTime([], { lat: 52, lon: 10 })).toBeNull();
  expect(nearestTrackTime([[]], { lat: 52, lon: 10 })).toBeNull();
});

test('die Videostelle zählt ab dem Beginn des Videos, davor ist es der Anfang', () => {
  expect(videoSecond(12_500, 10_000)).toBe(2.5);
  expect(videoSecond(9_000, 10_000)).toBe(0);
});
