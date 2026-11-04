// A small fake of the WebExtension API, enough for the extension's modules
// to load unchanged under Node: storage.local and storage.session backed by
// maps, alarms, tab/window/idle/runtime event emitters and action badges.
// Install it with `installFake()` before importing a module from src/.

import { vi } from "vitest";

type Listener = (...args: never[]) => unknown;

export class Emitter<F extends Listener> {
  listeners: F[] = [];
  addListener = (fn: F) => {
    this.listeners.push(fn);
  };
  removeListener = (fn: F) => {
    this.listeners = this.listeners.filter((l) => l !== fn);
  };
  /** Calls every listener and waits for the promises they return. */
  async fire(...args: Parameters<F>): Promise<void> {
    await Promise.all(this.listeners.map((l) => l(...args)));
  }
}

class Area {
  constructor(public data = new Map<string, unknown>()) {}
  get = async (keys?: string | string[] | null) => {
    const names = keys == null ? [...this.data.keys()] : Array.isArray(keys) ? keys : [keys];
    const out: Record<string, unknown> = {};
    for (const k of names) if (this.data.has(k)) out[k] = structuredClone(this.data.get(k));
    return out;
  };
  set = async (items: Record<string, unknown>) => {
    for (const [k, v] of Object.entries(items)) this.data.set(k, structuredClone(v));
  };
  remove = async (keys: string | string[]) => {
    for (const k of Array.isArray(keys) ? keys : [keys]) this.data.delete(k);
  };
  clear = async () => {
    this.data.clear();
  };
}

export interface FakeTab {
  id: number;
  windowId: number;
  active: boolean;
  url?: string;
  title?: string;
  incognito: boolean;
}

/** Everything a test drives or inspects. Storage maps can be handed to the
 * next `installFake` to simulate a worker restart. */
export function installFake(opts: { local?: Map<string, unknown>; session?: Map<string, unknown> } = {}) {
  const tabs = new Map<number, FakeTab>();
  const state = {
    focusedWindow: 1 as number | null,
    /** Windows whose tabs the extension cannot read (private, not allowed). */
    unreadable: new Set<number>(),
    badge: "",
    alarms: new Map<string, unknown>(),
    incognitoAllowed: false,
  };
  const api = {
    storage: { local: new Area(opts.local), session: new Area(opts.session) },
    alarms: {
      create: (name: string, info: unknown) => void state.alarms.set(name, info),
      onAlarm: new Emitter<(a: { name: string }) => unknown>(),
    },
    tabs: {
      get: async (id: number) => {
        const t = tabs.get(id);
        if (!t) throw new Error(`no tab ${id}`);
        return { ...t };
      },
      query: async (q: { active?: boolean; windowId?: number }) => {
        if (q.windowId !== undefined && state.unreadable.has(q.windowId)) return [];
        return [...tabs.values()]
          .filter((t) => (q.active === undefined || t.active === q.active) && (q.windowId === undefined || t.windowId === q.windowId))
          .map((t) => ({ ...t }));
      },
      onActivated: new Emitter<(i: { tabId: number; windowId: number }) => unknown>(),
      onUpdated: new Emitter<(id: number, change: { url?: string; title?: string }, tab: FakeTab) => unknown>(),
      onRemoved: new Emitter<(id: number, info: { windowId: number; isWindowClosing: boolean }) => unknown>(),
    },
    windows: {
      WINDOW_ID_NONE: -1,
      getLastFocused: async () => ({ id: state.focusedWindow ?? 1, focused: state.focusedWindow !== null }),
      onFocusChanged: new Emitter<(id: number) => unknown>(),
    },
    idle: { onStateChanged: new Emitter<(s: string) => unknown>() },
    runtime: {
      getManifest: () => ({ version: "0.2.0" }),
      onMessage: new Emitter<(m: unknown) => unknown>(),
      onInstalled: new Emitter<() => unknown>(),
      onStartup: new Emitter<() => unknown>(),
      sendMessage: vi.fn(async () => undefined),
      openOptionsPage: vi.fn(async () => undefined),
    },
    action: {
      setBadgeText: async ({ text }: { text: string }) => {
        state.badge = text;
      },
    },
    extension: { isAllowedIncognitoAccess: async () => state.incognitoAllowed },
  };
  (globalThis as Record<string, unknown>).chrome = api;
  delete (globalThis as Record<string, unknown>).browser;
  return { api, tabs, state };
}

export type Fake = ReturnType<typeof installFake>;

/** Lets pending promise chains and zero-delay timers run. */
export async function settle(rounds = 10): Promise<void> {
  for (let i = 0; i < rounds; i++) await new Promise((r) => setTimeout(r, 0));
}

export interface Call {
  url: string;
  method: string;
  body: unknown;
  headers: Record<string, string>;
}

type Reply = { status?: number; body?: unknown } | "refuse" | "hang";

/** Replaces fetch with a router over `http://127.0.0.1:<port>/v1<path>`.
 * `app` answers per port and path; ports it returns undefined for refuse. */
export function fakeFetch(app: (port: number, path: string, call: Call) => Reply | undefined) {
  const calls: Call[] = [];
  const fn = vi.fn(async (url: string, init: RequestInit = {}) => {
    const call: Call = {
      url,
      method: init.method ?? "GET",
      body: typeof init.body === "string" ? JSON.parse(init.body) : undefined,
      headers: (init.headers ?? {}) as Record<string, string>,
    };
    calls.push(call);
    const m = /^http:\/\/127\.0\.0\.1:(\d+)\/v1(\/.*)$/.exec(url);
    const reply = m ? app(Number(m[1]), m[2], call) : undefined;
    if (reply === undefined || reply === "refuse") throw new TypeError("Failed to fetch");
    if (reply === "hang") {
      return new Promise<Response>((_, reject) => {
        init.signal?.addEventListener("abort", () => reject(new DOMException("timeout", "TimeoutError")));
      });
    }
    return new Response(JSON.stringify(reply.body ?? {}), {
      status: reply.status ?? 200,
      headers: { "Content-Type": "application/json" },
    });
  });
  vi.stubGlobal("fetch", fn);
  return calls;
}

/** An app on `port` that accepts everything and answers /health. */
export function appOn(port: number, extra: (path: string, call: Call) => Reply | undefined = () => undefined) {
  return (p: number, path: string, call: Call): Reply | undefined => {
    if (p !== port) return undefined;
    if (path === "/health") return { body: { name: "nagsworth" } };
    return extra(path, call) ?? { body: { ok: true } };
  };
}
