import { screen } from '@testing-library/react';
import { MissionBar } from './MissionBar';
import { renderWithApp } from '../test/render';

test('ohne Auswahl zeigt die Leiste den laufenden Einsatz mit Zustand und Abschnitt', async () => {
  renderWithApp(<MissionBar />);
  expect(await screen.findAllByText('Übung Wietze')).not.toHaveLength(0);
  expect(screen.getByText('Laufend')).toBeInTheDocument();
  expect(screen.getByText('Abschnitt Nord')).toBeInTheDocument();
});

test('?einsatz=ohne zeigt „ohne Einsatz“', async () => {
  renderWithApp(<MissionBar />, { route: '/?einsatz=ohne' });
  expect(await screen.findAllByText('ohne Einsatz')).not.toHaveLength(0);
  expect(screen.queryByText('Abschnitt Nord')).not.toBeInTheDocument();
});
