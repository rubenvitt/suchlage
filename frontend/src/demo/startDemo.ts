import { meAdmin, meKiosk, meOperator } from '../contract/fixtures.generated';
import { handlersFor, missionFixtures } from '../test/handlers';
import { demoFlugmappeMission } from './missionData';

/**
 * Vorschau ohne Backend: MSW im Browser. Unbekannte Anfragen (Kartenkacheln, Glyphen) gehen ungehindert
 * raus. `?rolle=kiosk` oder `?rolle=operator` zeigt die Oberfläche in dieser Rolle, sonst Verwaltung.
 */
export async function startDemo() {
  const role = new URLSearchParams(window.location.search).get('rolle');
  const me = role === 'kiosk' ? meKiosk : role === 'operator' ? meOperator : meAdmin;
  const { setupWorker } = await import('msw/browser');
  await setupWorker(...handlersFor(me, [...missionFixtures(), demoFlugmappeMission()])).start({
    onUnhandledRequest: 'bypass',
    quiet: true,
  });
}
