import { useId, useState } from 'react';
import { Paneel, PaneelZustand, Segmentleiste } from '../components/instrument';
import { useAuthOptions, useSettings } from '../api/queries';
import { Frame } from '../shell/Frame';
import './admin.css';
import { KioskTokensPanel } from './KioskTokensPanel';
import { RemoteControllersPanel } from './RemoteControllersPanel';
import {
  BrokerForm,
  DjiForm,
  FlugmappeForm,
  GeneralForm,
  MediaForm,
  OidcForm,
  PhotoStorageForm,
} from './SettingsForms';
import { zustandOf } from './zustand';

const SECTIONS = [
  { wert: 'rc', label: 'Fernsteuerungen' },
  { wert: 'kiosk', label: 'Großbild' },
  { wert: 'oidc', label: 'Anmeldung' },
  { wert: 'dji', label: 'DJI' },
  { wert: 'broker', label: 'Broker' },
  { wert: 'media', label: 'Video' },
  { wert: 'photo_storage', label: 'Fotospeicher' },
  { wert: 'flugmappe', label: 'Flugmappe' },
  { wert: 'general', label: 'Allgemein' },
] as const;
type Section = (typeof SECTIONS)[number]['wert'];

/** Was jeder Abschnitt regelt, als Meta im Paneelkopf. */
const META: Record<Section, string> = {
  rc: 'Kopplung mit Pilot 2',
  kiosk: 'Bildschirme ohne Anmeldung',
  oidc: 'Pocket ID',
  dji: 'App-Daten aus dem DJI-Entwicklerportal',
  broker: 'MQTT im VPN',
  media: 'MediaMTX',
  photo_storage: 'Garage für die Fotos aus Pilot 2',
  flugmappe: 'Einsätze aus der Flugmappe',
  general: 'Verhalten im Einsatz',
};

function SettingsSection({ section, title }: { section: Section; title: string }) {
  const settings = useSettings();
  const redirectUrl = useAuthOptions().data?.redirect_url;
  if (section === 'rc') return <RemoteControllersPanel />;
  if (section === 'kiosk') return <KioskTokensPanel />;
  const { data } = settings;
  return (
    // Einstellungen sind ein Objekt, nie leer: `leerText` bleibt ungenutzt.
    <PaneelZustand
      zustand={zustandOf(settings)}
      titel={title}
      leerText=""
      onNeuladen={() => void settings.refetch()}
    >
      {data && section === 'oidc' && <OidcForm view={data.oidc} redirectUrl={redirectUrl} />}
      {data && section === 'dji' && <DjiForm view={data.dji} />}
      {data && section === 'broker' && <BrokerForm view={data.broker} />}
      {data && section === 'media' && <MediaForm view={data.media} />}
      {data && section === 'photo_storage' && <PhotoStorageForm view={data.photo_storage} />}
      {data && section === 'flugmappe' && <FlugmappeForm view={data.flugmappe} />}
      {data && section === 'general' && <GeneralForm view={data.general} />}
    </PaneelZustand>
  );
}

export function SettingsPage() {
  const [section, setSection] = useState<Section>('rc');
  const panelId = useId();
  const title = SECTIONS.find((s) => s.wert === section)!.label;
  return (
    <Frame>
      <div className="sl-page sl-settings">
        <Segmentleiste
          beschriftung="Bereich der Einstellungen"
          rolle="tablist"
          optionen={SECTIONS.map((s) => ({ ...s, steuert: panelId }))}
          wert={section}
          onWechsel={setSection}
          style={{ justifySelf: 'start' }}
        />
        <div id={panelId} role="tabpanel" aria-label={title}>
          <Paneel titel={title} meta={META[section]} koerperPolster>
            {/* `key`: ein Wechsel des Abschnitts beginnt mit frischem Formular und ohne alte Meldung. */}
            <SettingsSection key={section} section={section} title={title} />
          </Paneel>
        </div>
      </div>
    </Frame>
  );
}
