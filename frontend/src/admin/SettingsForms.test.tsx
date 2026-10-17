import { screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { http, HttpResponse } from 'msw';
import { settingsView } from '../contract/fixtures.generated';
import { server } from '../test/server';
import { renderWithApp } from '../test/render';
import { BrokerForm, PhotoStorageForm, secretOrNull } from './SettingsForms';

test('leeres Geheimnis behält den gespeicherten Wert', () => {
  expect(secretOrNull('')).toBeNull();
  expect(secretOrNull('  ')).toBeNull();
  expect(secretOrNull(undefined)).toBeNull();
  expect(secretOrNull('neu')).toBe('neu');
});

test('Broker speichern sendet null für das unveränderte Passwort und meldet Erfolg', async () => {
  let body: unknown;
  server.use(
    http.put('/api/settings/broker', async ({ request }) => {
      body = await request.json();
      return HttpResponse.json(settingsView);
    }),
  );
  renderWithApp(<BrokerForm view={settingsView.broker} />);
  expect(screen.getByLabelText('Admin-Passwort')).toHaveAttribute(
    'placeholder',
    'gespeichert – leer lassen zum Behalten',
  );
  await userEvent.click(screen.getByRole('button', { name: 'Broker speichern' }));
  expect(await screen.findByText('Gespeichert.')).toBeInTheDocument();
  expect(body).toEqual({
    host: 'mqtt',
    port: 1883,
    admin_username: 'suchlage-admin',
    admin_password: null,
    rc_url: 'tcp://100.64.0.10:1883',
  });
});

test('ein neues Passwort geht mit und das Feld ist danach wieder leer', async () => {
  let body: unknown;
  server.use(
    http.put('/api/settings/broker', async ({ request }) => {
      body = await request.json();
      return HttpResponse.json(settingsView);
    }),
  );
  renderWithApp(<BrokerForm view={settingsView.broker} />);
  await userEvent.type(screen.getByLabelText('Admin-Passwort'), 'neu-geheim');
  await userEvent.click(screen.getByRole('button', { name: 'Broker speichern' }));
  expect(await screen.findByText('Gespeichert.')).toBeInTheDocument();
  expect(body).toMatchObject({ admin_password: 'neu-geheim' });
  // Das Zurücksetzen hängt das Feld neu ein: neu abfragen.
  expect(screen.getByLabelText('Admin-Passwort')).toHaveValue('');
});

test('Fehler des Servers stehen am Formular', async () => {
  server.use(
    http.put('/api/settings/broker', () =>
      HttpResponse.json({ error: 'Adresse für die Fernsteuerung muss mit tcp:// beginnen' }, { status: 400 }),
    ),
  );
  renderWithApp(<BrokerForm view={settingsView.broker} />);
  await userEvent.click(screen.getByRole('button', { name: 'Broker speichern' }));
  expect(await screen.findByText(/muss mit tcp:\/\/ beginnen/)).toBeInTheDocument();
  expect(screen.queryByText('Gespeichert.')).not.toBeInTheDocument();
});

test('ohne gespeichertes Passwort gibt es keinen Behalten-Hinweis', () => {
  renderWithApp(<BrokerForm view={null} />);
  expect(screen.getByLabelText('Admin-Passwort')).not.toHaveAttribute('placeholder');
});

test('Fotospeicher speichern behält beide Geheimnisse, wenn die Felder leer bleiben', async () => {
  let body: unknown;
  server.use(
    http.put('/api/settings/photo_storage', async ({ request }) => {
      body = await request.json();
      return HttpResponse.json(settingsView);
    }),
  );
  renderWithApp(<PhotoStorageForm view={settingsView.photo_storage} />);
  expect(screen.getByLabelText('Admin-Token')).toHaveAttribute(
    'placeholder',
    'gespeichert – leer lassen zum Behalten',
  );
  await userEvent.click(screen.getByRole('button', { name: 'Fotospeicher speichern' }));
  expect(await screen.findByText('Gespeichert.')).toBeInTheDocument();
  expect(body).toEqual({
    s3_url: 'http://fotos:3900',
    rc_url: 'http://100.64.0.10:3900',
    admin_url: 'http://fotos:3903',
    bucket: 'suchlage-fotos',
    region: 'garage',
    access_key_id: 'GK31c2f218a2e44f485b94239e',
    secret_access_key: null,
    admin_token: null,
  });
});
