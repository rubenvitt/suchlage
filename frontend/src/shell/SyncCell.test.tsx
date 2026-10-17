import { screen } from '@testing-library/react';
import { snapshotFlying } from '../contract/fixtures.generated';
import type { BrokerStatus } from '../contract/generated/BrokerStatus';
import type { ConnectionStatus } from '../live/connection';
import { LiveProvider } from '../live/LiveProvider';
import { createLiveStore } from '../live/situationStore';
import { renderWithApp } from '../test/render';
import { SyncCell } from './SyncCell';

function show(status: ConnectionStatus, broker: BrokerStatus = 'connected') {
  const store = createLiveStore();
  store.dispatch({
    type: 'snapshot',
    snapshot: { ...snapshotFlying, system: { ...snapshotFlying.system, broker } },
  });
  store.setStatus(status);
  renderWithApp(
    <LiveProvider store={store}>
      <SyncCell />
    </LiveProvider>,
  );
}

const chip = (word: string) => screen.getByText(word).closest('[data-lfh="status-chip"]');

test.each<[ConnectionStatus, BrokerStatus, string, string]>([
  ['open', 'connected', 'SYNC', 'normal'],
  ['connecting', 'connected', 'verbindet', 'neutral'],
  ['reconnecting', 'disconnected', 'GETRENNT', 'alarm'],
  ['open', 'connecting', 'Broker verbindet', 'neutral'],
  ['open', 'disconnected', 'Broker getrennt', 'alarm'],
  ['open', 'not_configured', 'Broker nicht eingerichtet', 'achtung'],
])('Live-Strom %s, Broker %s: „%s“ in Ton %s', (status, broker, word, tone) => {
  show(status, broker);
  expect(chip(word)).toHaveAttribute('data-ton', tone);
});
