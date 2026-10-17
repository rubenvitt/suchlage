import { screen, within } from '@testing-library/react';
import { snapshotFlying } from '../contract/fixtures.generated';
import { renderWithApp } from '../test/render';
import { TelemetryStrip } from './TelemetryStrip';

const a = snapshotFlying.aircraft[0]!;
const props = {
  aircraft: a,
  gateway: snapshotFlying.gateways[0],
  flight: snapshotFlying.active_flights[0]!.flight,
  thresholds: snapshotFlying.freshness,
  size: 'compact' as const,
};

test('frische Werte ohne Schraffur', () => {
  const { container } = renderWithApp(<TelemetryStrip {...props} serverNow={a.last_seen_ms + 1_000} />);
  expect(container.querySelector('[data-freshness="fresh"]')).toBeInTheDocument();
  expect(screen.getByText('1 s')).toBeInTheDocument();
});

test('verzögerte Werte tragen ihr Alter', () => {
  const { container } = renderWithApp(<TelemetryStrip {...props} serverNow={a.last_seen_ms + 9_400} />);
  expect(container.querySelector('[data-freshness="delayed"]')).toBeInTheDocument();
  expect(screen.getByText('vor 9 s')).toBeInTheDocument();
});

test('verlorene Verbindung nennt die Uhrzeit der letzten Position', () => {
  renderWithApp(<TelemetryStrip {...props} serverNow={a.last_seen_ms + 45_000} />);
  expect(screen.getByText(/^Verbindung verloren \d{2}:\d{2}:\d{2}$/)).toBeInTheDocument();
});

test('ein neuer Wert ist ein neues Element und blitzt damit einmal auf', () => {
  const { rerender } = renderWithApp(<TelemetryStrip {...props} serverNow={a.last_seen_ms + 1_000} />);
  const before = screen.getByText('274');
  const turned = { ...a, telemetry: { ...a.telemetry, heading_deg: 280 } };
  rerender(<TelemetryStrip {...props} aircraft={turned} serverNow={a.last_seen_ms + 2_000} />);
  expect(screen.getByText('280')).not.toBe(before);
  expect(screen.getByText('280')).toHaveClass('sl-strip-flash');
});

test('beim ersten Bild und beim Ticken der Uhr blitzt nichts', () => {
  const { rerender } = renderWithApp(<TelemetryStrip {...props} serverNow={a.last_seen_ms + 1_000} />);
  const heading = screen.getByText('274');
  const flightCell = screen.getByText('Flugzeit').closest('.sl-strip-cell') as HTMLElement;
  expect(heading).not.toHaveClass('sl-strip-flash');
  rerender(<TelemetryStrip {...props} serverNow={a.last_seen_ms + 2_000} />);
  expect(screen.getByText('274')).toBe(heading);
  expect(within(flightCell).getByText('10:02')).not.toHaveClass('sl-strip-flash');
});

test('der Vorleser hört nur den Wechsel der Frische, nicht jede Sekunde', () => {
  const { rerender } = renderWithApp(<TelemetryStrip {...props} serverNow={a.last_seen_ms + 1_000} />);
  const live = screen.getByRole('status');
  expect(live).toHaveTextContent('Telemetrie aktuell');
  rerender(<TelemetryStrip {...props} serverNow={a.last_seen_ms + 9_400} />);
  expect(live).toHaveTextContent('Telemetrie verzögert');
  rerender(<TelemetryStrip {...props} serverNow={a.last_seen_ms + 10_400} />);
  expect(live).toHaveTextContent('Telemetrie verzögert');
  rerender(<TelemetryStrip {...props} serverNow={a.last_seen_ms + 45_000} />);
  expect(live).toHaveTextContent('Telemetrie verloren');
});

test('mobil zeigt nur die gewählten Werte und immer das Alter', () => {
  renderWithApp(
    <TelemetryStrip {...props} size="mobile" only={['elevation', 'battery']} serverNow={a.last_seen_ms} />,
  );
  expect(screen.getByText('Höhe ü. Start')).toBeInTheDocument();
  expect(screen.getByText('Akku')).toBeInTheDocument();
  expect(screen.queryByText('Kurs')).not.toBeInTheDocument();
  expect(screen.getByText('Alter')).toBeInTheDocument();
});

test('ohne Fluggerät keine Schraffur und keine Einheiten hinter Strichen', () => {
  const { container } = renderWithApp(
    <TelemetryStrip {...props} aircraft={undefined} serverNow={a.last_seen_ms} />,
  );
  const strip = container.querySelector('.sl-strip')!;
  expect(strip).not.toHaveClass('sl-hatch-lost');
  expect(within(strip as HTMLElement).queryByText('m/s')).not.toBeInTheDocument();
});
