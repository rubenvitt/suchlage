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
import type { MapElement } from '../contract/generated/MapElement';
import type { PhotoShot } from '../contract/generated/PhotoShot';
import type { TrackPoint } from '../contract/generated/TrackPoint';
import { photoCollection } from '../photos/photos';
import { basemapStyle } from './basemapStyle';
import { draftCollection, elementCollection, elementLabelCollection, type Draft } from './elements';
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
  /** Kartenelemente (M2); das Flugdetail zeigt keine. */
  elements?: MapElement[];
  selectedElementId?: string | null;
  /** Form im Entstehen: Die Karte zeigt sie, ein Doppelklick zoomt dann nicht. */
  draft?: Draft | null;
  /** Klick in die Karte: die Koordinate und das getroffene Element, sonst `null`. Ohne: keine Bedienung. */
  onMapClick?: (point: GeoPoint, elementId: string | null) => void;
  /** Auslösungen mit Aufnahmeort (M3); ohne Position fallen sie weg. */
  photos?: PhotoShot[];
  selectedPhotoId?: string | null;
  /** Klick auf ein Foto; ohne sind Fotos nur zu sehen (Großbild). */
  onPhotoClick?: (shotId: string) => void;
  /** Klick auf die Spur: die Koordinate. Ohne ist die Spur nicht anklickbar. */
  onTrackClick?: (point: GeoPoint) => void;
}

type Source = 'track' | 'aircraft' | 'gateways' | 'home' | 'elements' | 'element-labels' | 'draft' | 'photos';
type View = { center: maplibregl.LngLat; zoom: number };

const EMPTY: Collection<never, never> = { type: 'FeatureCollection', features: [] };
/** Deutschland im Überblick, bis die erste Position bekannt ist. */
const OVERVIEW = { center: [10.45, 51.16] as [number, number], zoom: 5 };
const FIT = { padding: 64, maxZoom: 16, duration: 0 };
const DEFAULT_LABEL = 'Lagekarte';
const NO_ELEMENTS: MapElement[] = [];
const NO_PHOTOS: PhotoShot[] = [];
/** Ebenen der Fotos, die ein- und ausgeblendet werden. */
const PHOTO_LAYERS = ['photos', 'photos-label'];
/** Ebenen, die ein Klick trifft; die oberste zuerst. */
const ELEMENT_LAYERS = ['elements-point', 'elements-line', 'elements-fill'];
/** Ein Klick daneben trifft noch: halbe Kantenlänge des Suchfelds in Pixeln. */
const HIT_PX = 6;
const POLYGON = ['==', ['geometry-type'], 'Polygon'] as maplibregl.FilterSpecification;
const POINT = ['==', ['geometry-type'], 'Point'] as maplibregl.FilterSpecification;
const NOT_POINT = ['!=', ['geometry-type'], 'Point'] as maplibregl.FilterSpecification;
const LINE = ['==', ['geometry-type'], 'LineString'] as maplibregl.FilterSpecification;
/** MapLibres Bedienhilfen sind englisch; nur diese drei erscheinen ohne eigene Steuerelemente. */
const LOCALE = {
  'Map.Title': 'Karte',
  'AttributionControl.ToggleAttribution': 'Quellenangabe ein- oder ausblenden',
  'AttributionControl.MapFeedback': 'Rückmeldung zur Karte',
};

const hitBox = ({ x, y }: maplibregl.Point): [maplibregl.PointLike, maplibregl.PointLike] => [
  [x - HIT_PX, y - HIT_PX],
  [x + HIT_PX, y + HIT_PX],
];

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
  elements = NO_ELEMENTS,
  selectedElementId = null,
  draft = null,
  onMapClick,
  photos = NO_PHOTOS,
  selectedPhotoId = null,
  onPhotoClick,
  onTrackClick,
}: Props) {
  const container = useRef<HTMLDivElement>(null);
  const mapRef = useRef<maplibregl.Map | null>(null);
  // Überlebt den Neuaufbau beim Wechsel Hell/Dunkel: Wer draußen auf Hell schaltet, behält seinen Ausschnitt.
  const view = useRef<View | null>(null);
  // Die Lage wird einmal ins Bild geholt; danach bestimmt der Nutzer den Ausschnitt.
  const framed = useRef(false);
  const [loaded, setLoaded] = useState(false);
  const [showPhotos, setShowPhotos] = useState(true);
  const [overTrack, setOverTrack] = useState(false);
  const { rollen, dunkel } = useRollen();
  const stopFollowing = useEffectEvent(() => onFollowChange?.(false));
  // Beim Zeichnen zählt jeder Klick als Eckpunkt, auch auf einem vorhandenen Element.
  const clicked = useEffectEvent((e: maplibregl.MapMouseEvent) => {
    const box = hitBox(e.point);
    // Ein Foto geht vor: Es liegt über den Elementen. Beim Zeichnen zählt der Klick als Eckpunkt.
    if (!draft && onPhotoClick && showPhotos) {
      const photo = e.target.queryRenderedFeatures(box, { layers: ['photos'] })[0]?.properties?.id;
      if (typeof photo === 'string') return onPhotoClick(photo);
    }
    if (!draft && onTrackClick && e.target.queryRenderedFeatures(box, { layers: ['track'] }).length > 0) {
      return onTrackClick({ lat: e.lngLat.lat, lon: e.lngLat.lng });
    }
    if (!onMapClick) return;
    const hits = draft
      ? []
      : e.target.queryRenderedFeatures(box, { layers: ELEMENT_LAYERS });
    const id = hits[0]?.properties?.id;
    onMapClick({ lat: e.lngLat.lat, lon: e.lngLat.lng }, typeof id === 'string' ? id : null);
  });

  // Über einer anklickbaren Spur zeigt der Zeiger die Hand.
  const hovered = useEffectEvent((e: maplibregl.MapMouseEvent) => {
    const over = onTrackClick != null && e.target.queryRenderedFeatures(hitBox(e.point), { layers: ['track'] }).length > 0;
    setOverTrack(over);
  });

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
    map.on('click', (e) => clicked(e));
    map.on('mousemove', (e) => hovered(e));
    map.on('load', () => {
      const image = (id: string, data: ImageData) => map.addImage(id, data, { pixelRatio });
      image('aircraft-fresh', aircraftGlyph(rollen.bedien, false, 40 * pixelRatio));
      image('aircraft-delayed', aircraftGlyph(rollen.gedaempft, false, 40 * pixelRatio));
      image('aircraft-lost', aircraftGlyph(rollen.schwach, true, 40 * pixelRatio));
      image('home', homeGlyph(rollen.gedaempft, 24 * pixelRatio));
      registerUnitSign(map, pixelRatio);
      for (const id of [
        'elements',
        'element-labels',
        'track',
        'aircraft',
        'gateways',
        'home',
        'draft',
        'photos',
      ] satisfies Source[]) {
        map.addSource(id, { type: 'geojson', data: EMPTY });
      }
      // Kartenelemente liegen unter Spur und Fluggerät: Die Live-Lage bleibt immer sichtbar.
      map.addLayer({
        id: 'elements-fill',
        type: 'fill',
        source: 'elements',
        filter: POLYGON,
        paint: { 'fill-color': ['get', 'color'], 'fill-opacity': ['case', ['get', 'selected'], 0.3, 0.16] },
      });
      map.addLayer({
        id: 'elements-line',
        type: 'line',
        source: 'elements',
        filter: NOT_POINT,
        layout: { 'line-cap': 'round', 'line-join': 'round' },
        paint: { 'line-color': ['get', 'color'], 'line-width': ['case', ['get', 'selected'], 4, 2.5] },
      });
      map.addLayer({
        id: 'elements-point',
        type: 'circle',
        source: 'elements',
        filter: POINT,
        paint: {
          'circle-color': ['get', 'color'],
          'circle-radius': ['case', ['get', 'selected'], 9, 7],
          'circle-stroke-color': ['case', ['get', 'selected'], rollen.text, rollen.grund],
          'circle-stroke-width': 2,
        },
      });
      const labelPaint = {
        'text-color': rollen.text,
        'text-halo-color': rollen.grund,
        'text-halo-width': 1.5,
      };
      // Flächen zuerst: Symbole weiter oben im Stapel werden zuerst platziert, ein Punkt behält so
      // seinen Namen. Die Flächenbeschriftung weicht bei einer Kollision auf eine andere Seite aus.
      map.addLayer({
        id: 'elements-area-label',
        type: 'symbol',
        source: 'element-labels',
        filter: ['!=', ['get', 'kind'], 'point'],
        layout: {
          'text-field': ['get', 'name'],
          'text-font': ['noto_sans_regular'],
          'text-size': 12,
          'text-variable-anchor': ['center', 'top', 'bottom', 'left', 'right'],
          'text-radial-offset': 1.2,
        },
        paint: labelPaint,
      });
      map.addLayer({
        id: 'elements-point-label',
        type: 'symbol',
        source: 'element-labels',
        filter: ['==', ['get', 'kind'], 'point'],
        layout: {
          'text-field': ['get', 'name'],
          'text-font': ['noto_sans_regular'],
          'text-size': 12,
          'text-offset': [0, 1.1],
          'text-anchor': 'top',
        },
        paint: labelPaint,
      });
      map.addLayer({
        id: 'elements-line-label',
        type: 'symbol',
        source: 'elements',
        filter: LINE,
        layout: {
          'symbol-placement': 'line',
          'text-field': ['get', 'name'],
          'text-font': ['noto_sans_regular'],
          'text-size': 12,
        },
        paint: labelPaint,
      });
      // Fotos über den Elementen, unter Spur und Fluggerät: ein Ring je Auslösung.
      map.addLayer({
        id: 'photos',
        type: 'circle',
        source: 'photos',
        paint: {
          'circle-color': rollen.grund,
          'circle-radius': ['case', ['get', 'selected'], 8, 6],
          'circle-stroke-color': ['case', ['get', 'selected'], rollen.text, rollen.bedien],
          'circle-stroke-width': ['case', ['get', 'selected'], 3, 2],
        },
      });
      map.addLayer({
        id: 'photos-label',
        type: 'symbol',
        source: 'photos',
        filter: ['get', 'selected'],
        layout: {
          'text-field': ['get', 'time'],
          'text-font': ['noto_sans_regular'],
          'text-size': 12,
          'text-offset': [0, 1.1],
          'text-anchor': 'top',
        },
        paint: labelPaint,
      });
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
      // Die Form im Entstehen liegt über allem, gestrichelt in der Bedienfarbe.
      map.addLayer({
        id: 'draft-fill',
        type: 'fill',
        source: 'draft',
        filter: POLYGON,
        paint: { 'fill-color': rollen.bedien, 'fill-opacity': 0.12 },
      });
      map.addLayer({
        id: 'draft-line',
        type: 'line',
        source: 'draft',
        filter: NOT_POINT,
        paint: { 'line-color': rollen.bedien, 'line-width': 2, 'line-dasharray': [2, 1.5] },
      });
      map.addLayer({
        id: 'draft-vertex',
        type: 'circle',
        source: 'draft',
        filter: POINT,
        paint: {
          'circle-radius': 5,
          'circle-color': rollen.grund,
          'circle-stroke-color': rollen.bedien,
          'circle-stroke-width': 2,
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
    if (loaded) setData(mapRef.current!, 'elements', elementCollection(elements, selectedElementId));
  }, [loaded, elements, selectedElementId]);

  useEffect(() => {
    if (loaded) setData(mapRef.current!, 'element-labels', elementLabelCollection(elements));
  }, [loaded, elements]);

  useEffect(() => {
    if (loaded) setData(mapRef.current!, 'photos', photoCollection(photos, selectedPhotoId));
  }, [loaded, photos, selectedPhotoId]);

  useEffect(() => {
    if (!loaded) return;
    const map = mapRef.current!;
    for (const id of PHOTO_LAYERS) map.setLayoutProperty(id, 'visibility', showPhotos ? 'visible' : 'none');
  }, [loaded, showPhotos]);

  const drafting = draft != null;
  useEffect(() => {
    if (!loaded) return;
    const map = mapRef.current!;
    setData(map, 'draft', draftCollection(draft));
  }, [loaded, draft]);

  useEffect(() => {
    if (!loaded) return;
    const zoom = mapRef.current!.doubleClickZoom;
    if (drafting) zoom.disable();
    else zoom.enable();
  }, [loaded, drafting]);

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
    // Ohne Live-Lage holen die Kartenelemente den Einsatzort ins Bild.
    const live = boundsOf([...tracks.flat(), positions(aircraft, gateways)]);
    const b = live ?? boundsOf(elements.map((e) => e.points));
    if (b) {
      map.fitBounds(b, FIT);
      framed.current = true;
    }
  }, [loaded, follow, aircraft, gateways, staticFit, tracks, elements]);

  const map = () => mapRef.current;

  return (
    <div className="sl-map" role="region" aria-label={label} data-drawing={drafting} data-over-track={overTrack}>
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
          {photos.length > 0 && (
            <Button
              aria-pressed={showPhotos}
              type={showPhotos ? 'primary' : 'default'}
              onClick={() => setShowPhotos((v) => !v)}
            >
              Fotos
            </Button>
          )}
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
