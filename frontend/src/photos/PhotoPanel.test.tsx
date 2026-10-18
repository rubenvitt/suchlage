import { screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { photoShots } from '../contract/fixtures.generated';
import { renderWithApp } from '../test/render';
import { PhotoPanel } from './PhotoPanel';

const [three, pending] = photoShots as [(typeof photoShots)[0], (typeof photoShots)[0]];

test('Wärmebild zuerst, der Umschalter wechselt die Linse samt Download', async () => {
  renderWithApp(<PhotoPanel shot={three} canOpenOriginal onClose={() => {}} />);
  expect(screen.getByRole('img')).toHaveAttribute('src', '/api/photos/11/thumb?v=1792248620000');
  expect(screen.getByRole('img')).toHaveAccessibleName('Wärme: DJI_20261017143012_0001_T.JPG');
  await userEvent.click(screen.getByRole('radio', { name: 'Zoom' }));
  expect(screen.getByRole('img')).toHaveAttribute('src', '/api/photos/13/thumb?v=1792248620000');
  expect(screen.getByRole('link', { name: 'Herunterladen' })).toHaveAttribute(
    'href',
    '/api/photos/13/original?v=1792248620000&download=1',
  );
  expect(screen.getByRole('link', { name: 'Original öffnen' })).toHaveAttribute('href', '/api/photos/13/original?v=1792248620000');
});

test('ohne Konto kein Original; ohne Vorschaubild und Ort ein Hinweis; Schließen meldet sich', async () => {
  const onClose = vi.fn();
  renderWithApp(<PhotoPanel shot={pending} canOpenOriginal={false} onClose={onClose} />);
  expect(screen.queryByRole('link')).toBeNull();
  expect(screen.queryByRole('radiogroup')).toBeNull();
  expect(screen.getByText('Vorschaubild entsteht …')).toBeInTheDocument();
  expect(screen.getByText(/Ohne Aufnahmeort/)).toBeInTheDocument();
  await userEvent.click(screen.getByRole('button', { name: 'Foto schließen' }));
  expect(onClose).toHaveBeenCalled();
});
