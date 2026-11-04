// Background worker: turns tab and window changes into events, queues them
// and sends them to the desktop app. The app decides what counts (only while
// the browser is the foreground app and the user is not idle).

import { forgetPort, get, post, NotPaired } from "./api";
import { browserName, ext, settings } from "./browser";
import { badgeText, type HostUsage } from "./format";
import { drop, load, push, type EventKind, type TabEvent } from "./queue";

const BROWSER = browserName();
const FLUSH_AT = 20;
const BATCH = 200;
const MAX_BACKOFF_MS = 5 * 60_000;

// MV3 workers are stopped after about 30 s without events and lose module
// state, so the retry deadline lives in storage.session (cleared when the
// browser exits, never written to disk). A restarted worker then keeps
// waiting instead of probing a closed app at every flush alarm.
const RETRY_KEY = "retry";

interface Retry {
  at: number;
  backoffMs: number;
}

async function getRetry(): Promise<Retry> {
  const s = await ext.storage.session.get(RETRY_KEY);
  return (s[RETRY_KEY] as Retry | undefined) ?? { at: 0, backoffMs: 0 };
}

let flushing = false;

/** What the popup shows under "Now": the last tab reported (after the
 * incognito scrub), kept in storage.session only. */
async function remember(e: TabEvent) {
  if (e.kind === "tab_removed") return;
  await ext.storage.session.set({ last: { url: e.url, title: e.title, ts: e.ts } });
}

async function includeIncognito(): Promise<boolean> {
  const s = await ext.storage.local.get("includeIncognito");
  return s.includeIncognito === true;
}

function event(kind: EventKind, tab?: chrome.tabs.Tab, windowId?: number): TabEvent {
  return {
    browser: BROWSER,
    ts: Date.now(),
    kind,
    tab_id: tab?.id ?? -1,
    window_id: tab?.windowId ?? windowId ?? null,
    url: tab?.url ?? null,
    title: tab?.title ?? null,
    incognito: tab?.incognito ?? false,
  };
}

async function record(e: TabEvent) {
  // Incognito tabs never leave the browser unless the user opted in; the app
  // only learns that no normal tab is being looked at.
  if (e.incognito && !(await includeIncognito())) {
    e = { ...e, kind: "focus_lost", url: null, title: null, incognito: false };
  }
  await remember(e);
  const len = await push(e);
  if (len >= FLUSH_AT) void flush();
}

async function flush(): Promise<void> {
  if (flushing) return;
  flushing = true;
  try {
    const retry = await getRetry();
    if (Date.now() < retry.at) return;
    try {
      const { clientId } = await settings();
      for (;;) {
        const q = await load();
        if (q.length === 0) break;
        const batch = q.slice(0, BATCH);
        await post("/ext/events", { client_id: clientId, events: batch });
        await drop(batch.length);
      }
      if (retry.at || retry.backoffMs) await ext.storage.session.remove(RETRY_KEY);
      await ext.storage.local.set({ lastSent: Date.now() });
    } catch (e) {
      if (!(e instanceof NotPaired)) {
        const backoffMs = Math.min(MAX_BACKOFF_MS, Math.max(5_000, retry.backoffMs * 2));
        await ext.storage.session.set({ [RETRY_KEY]: { at: Date.now() + backoffMs, backoffMs } });
      }
    }
  } finally {
    flushing = false;
  }
}

async function heartbeat() {
  const { clientId, token } = await settings();
  if (!token) return;
  let active: TabEvent | null = null;
  try {
    const win = await ext.windows.getLastFocused();
    if (win.focused) {
      const [tab] = await ext.tabs.query({ active: true, windowId: win.id });
      if (tab && (!tab.incognito || (await includeIncognito()))) active = event("heartbeat", tab);
    }
  } catch {
    // No browser window.
  }
  if (active) await remember(active);
  try {
    await post("/ext/heartbeat", {
      client_id: clientId,
      browser: BROWSER,
      version: ext.runtime.getManifest().version,
      active,
    });
    await ext.storage.local.set({ lastSent: Date.now() });
  } catch {
    // The app may be closed; the queue keeps tab events until it is back.
  }
  await updateBadge();
}

/** Today's web time on the toolbar icon when the user turned the badge on. */
async function updateBadge() {
  const { badge } = await ext.storage.local.get("badge");
  let text = "";
  if (badge === true) {
    try {
      const { hosts } = await get<{ hosts?: HostUsage[] }>("/stats/web");
      text = badgeText((hosts ?? []).reduce((n, h) => n + h.ms, 0));
    } catch {
      text = "";
    }
  }
  await ext.action.setBadgeText({ text });
}

ext.tabs.onActivated.addListener(async ({ tabId }) => {
  try {
    await record(event("tab_activated", await ext.tabs.get(tabId)));
  } catch {
    // Tab closed in the meantime.
  }
});

// Fires for the active tab of every window, focused or not. The app ignores a
// tab_updated for a tab it is not counting, so this is sent as it is.
ext.tabs.onUpdated.addListener((_tabId, change, tab) => {
  if (tab.active && (change.url || change.title)) void record(event("tab_updated", tab));
});

ext.tabs.onRemoved.addListener((tabId, info) => {
  void record({ ...event("tab_removed", undefined, info.windowId), tab_id: tabId });
});

ext.windows.onFocusChanged.addListener(async (windowId) => {
  if (windowId === ext.windows.WINDOW_ID_NONE) {
    await record(event("focus_lost"));
    return;
  }
  // A private window the extension may not run in yields no tab (or throws):
  // the user is looking at something the app must not keep counting.
  let tab: chrome.tabs.Tab | undefined;
  try {
    [tab] = await ext.tabs.query({ active: true, windowId });
  } catch {
    tab = undefined;
  }
  await record(tab ? event("tab_activated", tab) : event("focus_lost", undefined, windowId));
});

// The desktop app has its own idle detection; going idle is just a good
// moment to deliver what is queued.
ext.idle.onStateChanged.addListener((state) => {
  if (state !== "active") void flush();
});

// MV3 workers sleep; alarms wake them. 30 s is Chrome's minimum period.
ext.alarms.create("flush", { periodInMinutes: 0.5 });
ext.alarms.create("heartbeat", { periodInMinutes: 1 });
ext.alarms.onAlarm.addListener((alarm) => {
  if (alarm.name === "flush") void flush();
  if (alarm.name === "heartbeat") void heartbeat();
});

ext.runtime.onMessage.addListener((msg: { type?: string }) => {
  if (msg?.type === "paired") {
    void heartbeat();
    void flush();
  }
  if (msg?.type === "badge") void updateBadge();
  if (msg?.type === "forgotten") {
    forgetPort();
    void ext.action.setBadgeText({ text: "" });
  }
});

// One heartbeat as soon as the worker runs, so the app knows the active tab
// within seconds of a browser start, an install or a worker restart instead
// of at the next minute alarm. Heartbeats are idempotent state on the host.
// onInstalled and onStartup are listened to so the browser starts the worker
// for them; the module-level call below already does the work once.
let started: Promise<void> | null = null;
function startup(): Promise<void> {
  started ??= heartbeat().then(() => flush());
  return started;
}
ext.runtime.onInstalled.addListener(() => void startup());
ext.runtime.onStartup.addListener(() => void startup());
void startup();
