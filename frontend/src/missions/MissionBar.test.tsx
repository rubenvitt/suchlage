import { screen } from '@testing-library/react';
import { delay, http, HttpResponse } from 'msw';
import { MissionBar } from './MissionBar';
import { renderWithApp } from '../test/render';
import { server } from '../test/server';

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

test('solange die Einsätze laden, zeigt die Leiste weder Auswahl noch „ohne Einsatz“', async () => {
  server.use(http.get('/api/missions', () => delay('infinite')));
  renderWithApp(<MissionBar />);
  expect(await screen.findByText('Einsätze werden geladen …')).toBeInTheDocument();
  expect(screen.queryByText('ohne Einsatz')).not.toBeInTheDocument();
  expect(screen.queryByRole('combobox', { name: 'Einsatz wählen' })).not.toBeInTheDocument();
});

test('scheitert der Abruf, sagt die Leiste das statt „ohne Einsatz“', async () => {
  server.use(http.get('/api/missions', () => new HttpResponse(null, { status: 500 })));
  renderWithApp(<MissionBar />);
  expect(await screen.findByText('Einsätze nicht geladen')).toBeInTheDocument();
  expect(screen.queryByText('ohne Einsatz')).not.toBeInTheDocument();
});
