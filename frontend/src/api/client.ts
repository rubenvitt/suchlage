import type { ApiError as ApiErrorBody } from '../contract/generated/ApiError';

/** Fehlerantwort der API: Statuscode und der deutsche Satz aus `{"error": …}`. */
export class ApiError extends Error {
  constructor(
    public readonly status: number,
    message: string,
  ) {
    super(message);
  }
}

async function parse<T>(res: Response): Promise<T> {
  if (res.status === 204) return undefined as T;
  const text = await res.text();
  const body = text ? (JSON.parse(text) as unknown) : undefined;
  if (!res.ok) {
    const message = (body as ApiErrorBody | undefined)?.error ?? `Anfrage fehlgeschlagen (${res.status})`;
    throw new ApiError(res.status, message);
  }
  return body as T;
}

export async function apiGet<T>(path: string): Promise<T> {
  return parse<T>(await fetch(path, { credentials: 'same-origin', headers: { accept: 'application/json' } }));
}

export async function apiSend<T>(
  method: 'POST' | 'PUT' | 'DELETE',
  path: string,
  body?: unknown,
): Promise<T> {
  return parse<T>(
    await fetch(path, {
      method,
      credentials: 'same-origin',
      headers: body === undefined ? {} : { 'content-type': 'application/json' },
      body: body === undefined ? undefined : JSON.stringify(body),
    }),
  );
}
