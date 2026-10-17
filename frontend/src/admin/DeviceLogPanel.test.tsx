import { act, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { deviceLog } from '../contract/fixtures.generated';
import type { DeviceLogEvent } from '../contract/generated/DeviceLogEvent';
import type { LiveConnectionOptions } from '../live/connection';
import { LiveProvider } from '../live/LiveProvider';
import { createLiveStore } from '../live/situationStore';
import { renderWithApp } from '../test/render';
import { DeviceLogPanel } from './DeviceLogPanel';

function fakeFeed() {
  const feed: { opts?: LiveConnectionOptions<DeviceLogEvent> } = {};
  const connect = (opts: LiveConnectionOptions<DeviceLogEvent>) => {
    feed.opts = opts;
    return () => {};
  };
  const send = (e: DeviceLogEvent) => act(() => feed.opts!.onEvent(e));
  return { connect, feed, send };
}

function renderPanel() {
  const { connect, feed, send } = fakeFeed();
  renderWithApp(
    <LiveProvider store={createLiveStore()}>
      <DeviceLogPanel connect={connect} />
    </LiveProvider>,
  );
  return { feed, send };
}

test('zeigt Verlauf und neue Einträge, neueste oben, und den Payload auf Klick', async () => {
  const { feed, send } = renderPanel();
  expect(feed.opts!.url).toBe('/api/device-log');
  expect(screen.getByText(/Noch nichts empfangen/)).toBeInTheDocument();
  act(() => feed.opts!.onStatus('open'));
  expect(screen.getByText('live')).toBeInTheDocument();

  send(deviceLog[0]!);
  send(deviceLog[1]!);
  const rows = screen.getAllByRole('row').slice(1);
  expect(rows[0]).toHaveTextContent('live_start_push: keine Antwort in 10 s');
  expect(rows[0]).toHaveAttribute('data-level', 'error');
  expect(rows.at(-1)).toHaveTextContent('Verbindung zu Suchlage: getrennt');
  expect(rows.at(-1)).toHaveTextContent('Pilot-Seite');

  const toggle = screen.getByRole('button', { name: 'Befehl live_start_push' });
  await userEvent.click(toggle);
  expect(toggle).toHaveAttribute('aria-expanded', 'true');
  expect(screen.getByText(/"method": "live_start_push"/)).toBeInTheDocument();
});

test('Anhalten friert die Liste ein und zählt, was neu ankommt', async () => {
  const { send } = renderPanel();
  send(deviceLog[0]!);
  await userEvent.click(screen.getByRole('button', { name: 'Anhalten' }));
  send(deviceLog[1]!);
  expect(screen.queryByText('live_start_push: keine Antwort in 10 s')).not.toBeInTheDocument();
  await userEvent.click(screen.getByRole('button', { name: 'Weiter (1 neu)' }));
  expect(screen.getByText('live_start_push: keine Antwort in 10 s')).toBeInTheDocument();
});

test('nur Warnungen und Fehler blendet den Rest aus; verpasste Einträge stehen im Kopf', async () => {
  const { send } = renderPanel();
  send(deviceLog[0]!);
  send(deviceLog[2]!);
  expect(screen.getByText('4 Einträge verpasst')).toBeInTheDocument();
  await userEvent.click(screen.getByRole('switch', { name: 'nur Warnungen und Fehler' }));
  expect(screen.queryByText('Befehl live_start_push')).not.toBeInTheDocument();
  expect(screen.getByText('Verbindung zu Suchlage: getrennt')).toBeInTheDocument();
});
