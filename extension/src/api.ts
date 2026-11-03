// Talks to the desktop app's local API. Extensions cannot read a discovery
// file, so the port is found by probing the app's short fallback list.

import { ext, settings } from "./browser";

export const PORTS = [47821, 47822, 47823, 47824];

const base = (port: number) => `http://127.0.0.1:${port}/v1`;

async function healthy(port: number): Promise<boolean> {
  try {
    const res = await fetch(`${base(port)}/health`, { signal: AbortSignal.timeout(1500) });
    const body = await res.json();
    return res.ok && body?.name === "activity_nagger";
  } catch {
    return false;
  }
}

/** The app's port: the remembered one if it answers, else the first that does. */
export async function findPort(): Promise<number | null> {
  const { port } = await settings();
  if (port && (await healthy(port))) return port;
  for (const p of PORTS) {
    if (p !== port && (await healthy(p))) {
      await ext.storage.local.set({ port: p });
      return p;
    }
  }
  return null;
}

export class NotPaired extends Error {}

export async function post<T>(path: string, body: unknown): Promise<T> {
  const { token } = await settings();
  if (!token) throw new NotPaired("not paired");
  const port = await findPort();
  if (port === null) throw new Error("activity_nagger is not running");
  const res = await fetch(base(port) + path, {
    method: "POST",
    headers: { "Content-Type": "application/json", Authorization: `Bearer ${token}` },
    body: JSON.stringify(body),
  });
  if (res.status === 401) {
    await ext.storage.local.remove("token");
    throw new NotPaired("token rejected");
  }
  if (!res.ok) throw new Error(`HTTP ${res.status}`);
  return res.json() as Promise<T>;
}

/** Exchanges a 6-digit code shown by the app for the API token. */
export async function pair(code: string): Promise<void> {
  const port = await findPort();
  if (port === null) throw new Error("activity_nagger is not running on this computer");
  const res = await fetch(`${base(port)}/pair`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ code }),
  });
  if (!res.ok) throw new Error("Wrong or expired code");
  const { token } = (await res.json()) as { token: string };
  await ext.storage.local.set({ token });
}
