// Background worker: turns tab and window changes into events, queues them
// and sends them to the desktop app. The app decides what counts (only while
// the browser is the foreground app and the user is not idle).

import { post, NotPaired } from "./api";
import { browserName, ext, settings } from "./browser";
import { drop, load, push, type EventKind, type TabEvent } from "./queue";

const BROWSER = browserName();
const FLUSH_AT = 20;
const BATCH = 200;
const MAX_BACKOFF_MS = 5 * 60_000;

let backoffMs = 0;
let retryAt = 0;
let flushing = false;

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
  const len = await push(e);
  if (len >= FLUSH_AT) void flush();
}

async function flush(): Promise<void> {
  if (flushing || Date.now() < retryAt) return;
  flushing = true;
  try {
    const { clientId } = await settings();
    for (;;) {
      const q = await load();
      if (q.length === 0) break;
      const batch = q.slice(0, BATCH);
      await post("/ext/events", { client_id: clientId, events: batch });
      await drop(batch.length);
    }
    backoffMs = 0;
    retryAt = 0;
  } catch (e) {
    if (!(e instanceof NotPaired)) {
      backoffMs = Math.min(MAX_BACKOFF_MS, Math.max(5_000, backoffMs * 2));
      retryAt = Date.now() + backoffMs;
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
  try {
    await post("/ext/heartbeat", {
      client_id: clientId,
      browser: BROWSER,
      version: ext.runtime.getManifest().version,
      active,
    });
  } catch {
    // The app may be closed; the queue keeps tab events until it is back.
  }
}

ext.tabs.onActivated.addListener(async ({ tabId }) => {
  try {
    await record(event("tab_activated", await ext.tabs.get(tabId)));
  } catch {
    // Tab closed in the meantime.
  }
});

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
  const [tab] = await ext.tabs.query({ active: true, windowId });
  if (tab) await record(event("tab_activated", tab));
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
});
