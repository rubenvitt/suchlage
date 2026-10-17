import { act, renderHook } from '@testing-library/react';
import { useWakeLock } from './useWakeLock';

function stubWakeLock() {
  const sentinels: { release: ReturnType<typeof vi.fn> }[] = [];
  let resolveNext: (() => void) | undefined;
  const request = vi.fn(
    () =>
      new Promise((resolve) => {
        const sentinel = { release: vi.fn(() => Promise.resolve()) };
        sentinels.push(sentinel);
        resolveNext = () => resolve(sentinel);
      }),
  );
  Object.defineProperty(navigator, 'wakeLock', { value: { request }, configurable: true });
  return { request, sentinels, grant: () => act(async () => resolveNext?.()) };
}

function setVisibility(state: DocumentVisibilityState) {
  Object.defineProperty(document, 'visibilityState', { value: state, configurable: true });
  act(() => {
    document.dispatchEvent(new Event('visibilitychange'));
  });
}

afterEach(() => {
  Reflect.deleteProperty(navigator, 'wakeLock');
  Reflect.deleteProperty(document, 'visibilityState');
});

test('hält den Bildschirm wach und gibt die Sperre beim Verlassen frei', async () => {
  const lock = stubWakeLock();
  const { unmount } = renderHook(() => useWakeLock());
  expect(lock.request).toHaveBeenCalledWith('screen');
  await lock.grant();
  unmount();
  expect(lock.sentinels[0]!.release).toHaveBeenCalled();
});

test('fordert die Sperre neu an, wenn die Seite wieder sichtbar wird', async () => {
  const lock = stubWakeLock();
  renderHook(() => useWakeLock());
  await lock.grant();
  // Der Browser gibt die Sperre selbst frei, sobald die Seite verdeckt ist.
  setVisibility('hidden');
  expect(lock.request).toHaveBeenCalledTimes(1);
  setVisibility('visible');
  expect(lock.request).toHaveBeenCalledTimes(2);
});

test('kommt die Sperre erst nach dem Verlassen, wird sie sofort freigegeben', async () => {
  const lock = stubWakeLock();
  const { unmount } = renderHook(() => useWakeLock());
  unmount();
  await lock.grant();
  expect(lock.sentinels[0]!.release).toHaveBeenCalled();
});

test('ohne Wake Lock im Browser läuft die Ansicht weiter', () => {
  expect('wakeLock' in navigator).toBe(false);
  const { unmount } = renderHook(() => useWakeLock());
  setVisibility('visible');
  unmount();
});

test('eine abgelehnte Anfrage bleibt still', async () => {
  const request = vi.fn(() => Promise.reject(new DOMException('nicht erlaubt', 'NotAllowedError')));
  Object.defineProperty(navigator, 'wakeLock', { value: { request }, configurable: true });
  const { unmount } = renderHook(() => useWakeLock());
  await act(async () => {});
  expect(request).toHaveBeenCalled();
  unmount();
});
