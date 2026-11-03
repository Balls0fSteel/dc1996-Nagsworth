// Persistent FIFO of tab events in storage.local, so events survive the
// service worker being stopped and the app being closed.

import { ext } from "./browser";

export type EventKind = "tab_activated" | "tab_updated" | "tab_removed" | "focus_lost" | "heartbeat";

export interface TabEvent {
  browser: string;
  ts: number;
  kind: EventKind;
  tab_id: number;
  window_id: number | null;
  url: string | null;
  title: string | null;
  incognito: boolean;
}

const KEY = "queue";
export const MAX_QUEUE = 5000;

// storage.local has no transactions; serialize read-modify-write cycles.
let chain: Promise<unknown> = Promise.resolve();
function locked<T>(fn: () => Promise<T>): Promise<T> {
  const next = chain.then(fn, fn);
  chain = next.catch(() => undefined);
  return next;
}

export async function load(): Promise<TabEvent[]> {
  const s = await ext.storage.local.get(KEY);
  return (s[KEY] as TabEvent[] | undefined) ?? [];
}

/** Appends an event, dropping the oldest beyond the cap. Returns the new length. */
export function push(e: TabEvent): Promise<number> {
  return locked(async () => {
    const q = await load();
    q.push(e);
    if (q.length > MAX_QUEUE) q.splice(0, q.length - MAX_QUEUE);
    await ext.storage.local.set({ [KEY]: q });
    return q.length;
  });
}

/** Removes the first `n` events after they were delivered. */
export function drop(n: number): Promise<void> {
  return locked(async () => {
    const q = await load();
    q.splice(0, n);
    await ext.storage.local.set({ [KEY]: q });
  });
}
