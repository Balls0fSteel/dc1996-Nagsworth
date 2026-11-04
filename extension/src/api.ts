// Talks to the desktop app's local API. Extensions cannot read a discovery
// file, so the port is found by probing the app's short fallback list.

import { ext, settings } from "./browser";

export const PORTS = [47821, 47822, 47823, 47824];
/** Per-probe timeout: the remembered port, then the rest in parallel, so a
 * closed app is reported within about two of these. */
export const PROBE_TIMEOUT_MS = 1000;

const base = (port: number) => `http://127.0.0.1:${port}/v1`;

async function healthy(port: number): Promise<boolean> {
  try {
    const res = await fetch(`${base(port)}/health`, { signal: AbortSignal.timeout(PROBE_TIMEOUT_MS) });
    const body = await res.json();
    return res.ok && body?.name === "nagsworth";
  } catch {
    return false;
  }
}

// The last port that answered, for the worker's (or page's) lifetime; only a
// connection failure makes `request` probe again.
let knownPort: number | null = null;

/** Forgets the in-memory port, as after "Forget everything". */
export function forgetPort(): void {
  knownPort = null;
}

/** The app's port: the remembered one if it answers, else the lowest of the
 * others that does, probed in parallel. */
export async function findPort(): Promise<number | null> {
  const { port } = await settings();
  if (port && (await healthy(port))) {
    knownPort = port;
    return port;
  }
  const others = PORTS.filter((p) => p !== port);
  const answers = await Promise.all(others.map(healthy));
  const found = others.find((_, i) => answers[i]) ?? null;
  if (found !== null && found !== port) await ext.storage.local.set({ port: found });
  knownPort = found;
  return found;
}

export class NotPaired extends Error {}
export class NotRunning extends Error {}

async function request<T>(method: "GET" | "POST", path: string, body?: unknown): Promise<T> {
  const { token } = await settings();
  if (!token) throw new NotPaired("not paired");
  const init: RequestInit = {
    method,
    headers: {
      Authorization: `Bearer ${token}`,
      ...(body === undefined ? {} : { "Content-Type": "application/json" }),
    },
    body: body === undefined ? undefined : JSON.stringify(body),
  };
  let res: Response | null = null;
  // Once with the known port, once more after a re-probe if that failed.
  for (let attempt = 0; attempt < 2 && res === null; attempt++) {
    const port = attempt === 0 && knownPort !== null ? knownPort : await findPort();
    if (port === null) throw new NotRunning("Nagsworth is not running");
    try {
      res = await fetch(base(port) + path, init);
    } catch (e) {
      knownPort = null;
      if (attempt === 1) throw new NotRunning(e instanceof Error ? e.message : String(e));
    }
  }
  if (res === null) throw new NotRunning("Nagsworth is not running");
  if (res.status === 401) {
    await ext.storage.local.remove("token");
    throw new NotPaired("token rejected");
  }
  if (!res.ok) throw new Error(`HTTP ${res.status}`);
  return res.json() as Promise<T>;
}

export const post = <T>(path: string, body: unknown = {}): Promise<T> => request<T>("POST", path, body);
export const get = <T>(path: string): Promise<T> => request<T>("GET", path);

/** Exchanges a 6-digit code shown by the app for the API token. */
export async function pair(code: string): Promise<void> {
  const port = await findPort();
  if (port === null) throw new NotRunning("Nagsworth is not running on this computer");
  const res = await fetch(`${base(port)}/pair`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ code }),
  });
  if (!res.ok) throw new Error("Wrong or expired code");
  const { token } = (await res.json()) as { token: string };
  await ext.storage.local.set({ token });
}
