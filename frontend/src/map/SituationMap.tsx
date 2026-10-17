import * as maplibregl from 'maplibre-gl';
import 'maplibre-gl/dist/maplibre-gl.css';
import './maplibreWorker';
import './map.css';
import { Button } from 'antd';
import { useEffect, useEffectEvent, useRef, useState } from 'react';
import { useRollen } from '../components/instrument';
import type { AircraftState } from '../contract/generated/AircraftState';
import type { FreshnessThresholds } from '../contract/generated/FreshnessThresholds';
import type { GatewayState } from '../contract/generated/GatewayState';
import type { GeoPoint } from '../contract/generated/GeoPoint';
import type { TrackPoint } from '../contract/generated/TrackPoint';
import { basemapStyle } from './basemapStyle';
import { MapBoundary } from './MapBoundary';
import { aircraftGlyph, homeGlyph } from './glyphs';
import {
  aircraftCollection,
  boundsOf,
  gatewayCollection,
  homeCollection,
  trackCollection,
  type Collection,
} from './layers';
import { registerUnitSign, UNIT_IMAGE } from './unitSign';

interface Props {
  gateways: GatewayState[];
  aircraft: AircraftState[];
  tracks: TrackPoint[][][];
  serverNow: number;
  thresholds: FreshnessThresholds;
  /** Karte folgt dem ersten Fluggerät. */
  follow?: boolean;
  onFollowChange?: (follow: boolean) => void;
  /** Flugdetail: einmal auf die Spur zoomen, keine Live-Gerätezeichen. */
  staticFit?: boolean;
  /** Folgen, Vergrößern, Verkleinern; das Großbild zeigt sie nicht, dort bedient niemand. */
  showTools?: boolean;
  label?: string;
}

type Source = 'track' | 'aircraft' | 'gateways' | 'home';
type View = { center: maplibregl.LngLat; zoom: number };

const EMPTY: Collection<never, never> = { type: 'FeatureCollection', features: [] };
/** Deutschland im Überblick, bis die erste Position bekannt ist. */
const OVERVIEW = { center: [10.45, 51.16] as [number, number], zoom: 5 };
const FIT = { padding: 64, maxZoom: 16, duration: 0 };
const DEFAULT_LABEL = 'Lagekarte';
/** MapLibres Bedienhilfen sind englisch; nur diese drei erscheinen ohne eigene Steuerelemente. */
const LOCALE = {
  'Map.Title': 'Karte',
  'AttributionControl.ToggleAttribution': 'Quellenangabe ein- oder ausblenden',
  'AttributionControl.MapFeedback': 'Rückmeldung zur Karte',
};

function setData<G, P>(map: maplibregl.Map, source: Source, data: Collection<G, P>) {
  map.getSource<maplibregl.GeoJSONSource>(source)?.setData(data);
}

const positions = (aircraft: AircraftState[], gateways: GatewayState[]): GeoPoint[] =>
  [...aircraft.map((a) => a.telemetry.position), ...gateways.map((g) => g.position)].filter(
    (p): p is GeoPoint => p !== null,
  );

/** Fehlt der Grafikkontext, steht im Bereich der Karte ein Hinweis; der Rest der Seite bleibt. */
export function SituationMap(props: Props) {
  return (
    <MapBoundary label={props.label ?? DEFAULT_LABEL}>
      <MapCanvas {...props} />
    </MapBoundary>
  );
}

function MapCanvas({
  gateways,
  aircraft,
  tracks,
  serverNow,
  thresholds,
  follow = false,
  onFollowChange,
  staticFit = false,
  showTools = true,
  label = DEFAULT_LABEL,
}: Props) {
  const container = useRef<HTMLDivElement>(null);
  const mapRef = useRef<maplibregl.Map | null>(null);
  // Überlebt den Neuaufbau beim Wechsel Hell/Dunkel: Wer draußen auf Hell schaltet, behält seinen Ausschnitt.
  const view = useRef<View | null>(null);
  // Die Lage wird einmal ins Bild geholt; danach bestimmt der Nutzer den Ausschnitt.
  const framed = useRef(false);
  const [loaded, setLoaded] = useState(false);
  const { rollen, dunkel } = useRollen();
  const stopFollowing = useEffectEvent(() => onFollowChange?.(false));

  useEffect(() => {
    const pixelRatio = window.devicePixelRatio || 1;
    const map = new maplibregl.Map({
      container: container.current!,
      style: basemapStyle(dunkel ? 'dark' : 'light'),
      ...(view.current ?? OVERVIEW),
      maxPitch: 0,
      touchPitch: false,
      dragRotate: false,
      attributionControl: { compact: true },
      locale: LOCALE,
    });
    // Norden bleibt oben: Der Kurspfeil ist nur so auf einen Blick lesbar.
    map.touchZoomRotate.disableRotation();
    map.keyboard.disableRotation();
    mapRef.current = map;
    // Wer die Karte selbst verschiebt, will dort hinsehen; Folgen würde ihn jede Sekunde zurückziehen.
    map.on('dragstart', (e) => {
      if (e.originalEvent) stopFollowing();
    });
    map.on('load', () => {
      const image = (id: string, data: ImageData) => map.addImage(id, data, { pixelRatio });
      image('aircraft-fresh', aircraftGlyph(rollen.bedien, false, 40 * pixelRatio));
      image('aircraft-delayed', aircraftGlyph(rollen.gedaempft, false, 40 * pixelRatio));
      image('aircraft-lost', aircraftGlyph(rollen.schwach, true, 40 * pixelRatio));
      image('home', homeGlyph(rollen.gedaempft, 24 * pixelRatio));
      registerUnitSign(map, pixelRatio);
      for (const id of ['track', 'aircraft', 'gateways', 'home'] satisfies Source[]) {
        map.addSource(id, { type: 'geojson', data: EMPTY });
      }
      map.addLayer({
        id: 'track',
        type: 'line',
        source: 'track',
        layout: { 'line-cap': 'round', 'line-join': 'round' },
        paint: { 'line-color': rollen.bedien, 'line-width': 3, 'line-opacity': 0.9 },
      });
      map.addLayer({
        id: 'home',
        type: 'symbol',
        source: 'home',
        layout: { 'icon-image': 'home', 'icon-allow-overlap': true },
      });
      // Das taktische Zeichen dreht und färbt sich nicht; sein Alter zeigen Deckkraft und Beschriftung.
      map.addLayer({
        id: 'gateways',
        type: 'symbol',
        source: 'gateways',
        layout: {
          'icon-image': UNIT_IMAGE,
          'icon-allow-overlap': true,
          // Ohne das fiele mit verdeckter Beschriftung das ganze Zeichen weg (Icon und Text gehören zusammen).
          'text-allow-overlap': true,
          'text-field': [
            'case',
            ['==', ['get', 'age'], ''],
            ['get', 'label'],
            ['concat', ['get', 'label'], ' · ', ['get', 'age']],
          ],
          'text-font': ['noto_sans_regular'],
          'text-size': 11,
          'text-offset': [0, 2],
          'text-anchor': 'top',
        },
        paint: {
          'icon-opacity': ['match', ['get', 'freshness'], 'lost', 0.35, 'delayed', 0.7, 1],
          'text-color': [
            'match',
            ['get', 'freshness'],
            'fresh',
            rollen.text,
            'lost',
            rollen.alarmText,
            rollen.achtungText,
          ],
          'text-halo-color': rollen.grund,
          'text-halo-width': 1.5,
        },
      });
      map.addLayer({
        id: 'aircraft',
        type: 'symbol',
        source: 'aircraft',
        layout: {
          'icon-image': ['concat', 'aircraft-', ['get', 'freshness']],
          'icon-rotate': ['get', 'heading'],
          'icon-rotation-alignment': 'map',
          'icon-allow-overlap': true,
          'text-allow-overlap': true,
          'text-field': ['get', 'age'],
          'text-font': ['noto_sans_regular'],
          'text-size': 12,
          'text-offset': [0, 1.8],
          'text-anchor': 'top',
        },
        paint: {
          'text-color': ['match', ['get', 'freshness'], 'lost', rollen.alarmText, rollen.achtungText],
          'text-halo-color': rollen.grund,
          'text-halo-width': 1.5,
        },
      });
      setLoaded(true);
    });
    return () => {
      view.current = { center: map.getCenter(), zoom: map.getZoom() };
      map.remove();
      mapRef.current = null;
      setLoaded(false);
    };
  }, [dunkel, rollen]);

  useEffect(() => {
    if (loaded) setData(mapRef.current!, 'track', trackCollection(tracks));
  }, [loaded, tracks]);

  useEffect(() => {
    if (loaded) setData(mapRef.current!, 'home', homeCollection(aircraft));
  }, [loaded, aircraft]);

  useEffect(() => {
    if (!loaded) return;
    const map = mapRef.current!;
    setData(map, 'aircraft', staticFit ? EMPTY : aircraftCollection(aircraft, serverNow, thresholds));
    setData(map, 'gateways', staticFit ? EMPTY : gatewayCollection(gateways, serverNow, thresholds));
  }, [loaded, aircraft, gateways, serverNow, thresholds, staticFit]);

  useEffect(() => {
    if (!loaded) return;
    const map = mapRef.current!;
    if (staticFit) {
      const b = boundsOf(tracks.flat());
      if (b) map.fitBounds(b, FIT);
      return;
    }
    const p = aircraft[0]?.telemetry.position;
    // Folgen verschiebt nur; den Maßstab setzt erst das Einpassen darunter, sonst bliebe die Übersicht stehen.
    if (follow && p && framed.current) {
      const reduced = window.matchMedia('(prefers-reduced-motion: reduce)').matches;
      map.easeTo({ center: [p.lon, p.lat], duration: reduced ? 0 : 400 });
      return;
    }
    if (framed.current) return;
    // Spur, Fluggerät und Trupp zusammen: So sieht man, wo der Trupp steht und wohin geflogen wird.
    const b = boundsOf([...tracks.flat(), positions(aircraft, gateways)]);
    if (b) {
      map.fitBounds(b, FIT);
      framed.current = true;
    }
  }, [loaded, follow, aircraft, gateways, staticFit, tracks]);

  const map = () => mapRef.current;

  return (
    <div className="sl-map" role="region" aria-label={label}>
      <div ref={container} className="sl-map-canvas" />
      {!staticFit && showTools && (
        <div className="sl-map-tools">
          <Button
            aria-pressed={follow}
            type={follow ? 'primary' : 'default'}
            onClick={() => onFollowChange?.(!follow)}
          >
            Folgen
          </Button>
          <Button aria-label="Vergrößern" onClick={() => map()?.zoomIn({ duration: 150 })}>
            +
          </Button>
          <Button aria-label="Verkleinern" onClick={() => map()?.zoomOut({ duration: 150 })}>
            −
          </Button>
        </div>
      )}
    </div>
  );
}
