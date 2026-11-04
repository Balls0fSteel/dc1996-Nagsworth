import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { appOn, fakeFetch, installFake, settle, type Call, type Fake } from "./fake";
import type { TabEvent } from "../src/queue";

let fake: Fake;

const UA = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/156.0.0.0 Safari/537.36 Edg/156.0.0.0";

/** Loads the worker as the browser would start it. */
async function startWorker() {
  vi.stubGlobal("navigator", { userAgent: UA });
  await import("../src/background");
  await settle();
}

const queued = () => (fake.api.storage.local.data.get("queue") as TabEvent[] | undefined) ?? [];

beforeEach(() => {
  vi.resetModules();
  fake = installFake();
  fake.api.storage.local.data.set("token", "tok");
  fake.tabs.set(10, { id: 10, windowId: 1, active: true, url: "https://example.org/a", title: "A", incognito: false });
  fake.tabs.set(20, { id: 20, windowId: 2, active: true, url: "https://secret.example/x", title: "Secret", incognito: true });
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("start-up", () => {
  it("sends one heartbeat with the active tab as soon as the worker runs", async () => {
    const calls = fakeFetch(appOn(47821));
    await startWorker();
    const hb = calls.filter((c) => c.url.endsWith("/ext/heartbeat"));
    expect(hb.length).toBe(1);
    expect(hb[0].body).toMatchObject({ browser: "edge", version: "0.2.0", active: { kind: "heartbeat", url: "https://example.org/a" } });
    // onInstalled / onStartup after the worker started do not send another.
    await fake.api.runtime.onStartup.fire();
    await fake.api.runtime.onInstalled.fire();
    await settle();
    expect(calls.filter((c) => c.url.endsWith("/ext/heartbeat")).length).toBe(1);
  });

  it("sends nothing while not paired", async () => {
    fake.api.storage.local.data.delete("token");
    const calls = fakeFetch(appOn(47821));
    await startWorker();
    expect(calls.filter((c) => !c.url.endsWith("/health"))).toEqual([]);
  });
});

describe("tracking edges", () => {
  beforeEach(() => {
    fakeFetch(appOn(47821));
  });

  it("records focus_lost when the focused window has no readable tab", async () => {
    await startWorker();
    fake.state.unreadable.add(3);
    await fake.api.windows.onFocusChanged.fire(3);
    await settle();
    expect(queued().at(-1)).toMatchObject({ kind: "focus_lost", window_id: 3, url: null, title: null });
  });

  it("records focus_lost when no browser window has focus", async () => {
    await startWorker();
    await fake.api.windows.onFocusChanged.fire(-1);
    expect(queued().at(-1)).toMatchObject({ kind: "focus_lost", url: null });
  });

  it("records the active tab when a readable window gets focus", async () => {
    await startWorker();
    await fake.api.windows.onFocusChanged.fire(1);
    expect(queued().at(-1)).toMatchObject({ kind: "tab_activated", tab_id: 10, url: "https://example.org/a" });
  });

  it("sends tab_updated from an unfocused window unchanged (the app ignores tabs it is not counting)", async () => {
    await startWorker();
    const tab = { id: 30, windowId: 9, active: true, url: "https://other.example/", title: "Other", incognito: false };
    await fake.api.tabs.onUpdated.fire(30, { url: tab.url }, tab);
    await settle();
    expect(queued().at(-1)).toMatchObject({ kind: "tab_updated", tab_id: 30, window_id: 9, url: "https://other.example/", title: "Other" });
  });
});

describe("incognito scrub", () => {
  beforeEach(() => {
    fakeFetch(appOn(47821));
  });

  it("turns a private tab event into focus_lost without url or title", async () => {
    await startWorker();
    await fake.api.tabs.onActivated.fire({ tabId: 20, windowId: 2 });
    await settle();
    const e = queued().at(-1);
    expect(e).toMatchObject({ kind: "focus_lost", url: null, title: null, incognito: false });
    expect(JSON.stringify(queued())).not.toContain("secret");
    expect(JSON.stringify(fake.api.storage.session.data.get("last"))).not.toContain("secret");
  });

  it("keeps a private tab when the user opted in", async () => {
    fake.api.storage.local.data.set("includeIncognito", true);
    await startWorker();
    await fake.api.tabs.onActivated.fire({ tabId: 20, windowId: 2 });
    await settle();
    expect(queued().at(-1)).toMatchObject({ kind: "tab_activated", url: "https://secret.example/x", incognito: true });
  });

  it("sends a heartbeat without the private tab", async () => {
    fake.state.focusedWindow = 2;
    const calls: Call[] = fakeFetch(appOn(47821));
    await startWorker();
    const hb = calls.find((c) => c.url.endsWith("/ext/heartbeat"));
    expect(hb?.body).toMatchObject({ active: null });
    expect(JSON.stringify(calls.map((c) => c.body))).not.toContain("secret");
  });
});

describe("delivery", () => {
  it("flushes the queue on the alarm and records when it was sent", async () => {
    const calls = fakeFetch(appOn(47821));
    await startWorker();
    await fake.api.tabs.onActivated.fire({ tabId: 10, windowId: 1 });
    await fake.api.alarms.onAlarm.fire({ name: "flush" });
    await settle();
    const sent = calls.filter((c) => c.url.endsWith("/ext/events"));
    expect(sent.length).toBe(1);
    expect((sent[0].body as { events: TabEvent[] }).events.map((e) => e.tab_id)).toEqual([10]);
    expect(queued()).toEqual([]);
    expect(typeof fake.api.storage.local.data.get("lastSent")).toBe("number");
  });

  it("keeps the retry deadline across a worker restart", async () => {
    let calls = fakeFetch(() => undefined); // the app is closed
    await startWorker();
    await fake.api.tabs.onActivated.fire({ tabId: 10, windowId: 1 });
    await fake.api.alarms.onAlarm.fire({ name: "flush" });
    await settle();
    const retry = fake.api.storage.session.data.get("retry") as { at: number; backoffMs: number };
    expect(retry.backoffMs).toBe(5_000);
    expect(retry.at).toBeGreaterThan(Date.now());

    // Chrome stops the worker; module state is gone, storage stays.
    const local = fake.api.storage.local.data;
    const session = fake.api.storage.session.data;
    vi.resetModules();
    fake = installFake({ local, session });
    calls = fakeFetch(appOn(47821)); // the app is back, but the deadline has not passed
    await startWorker();
    await fake.api.alarms.onAlarm.fire({ name: "flush" });
    await settle();
    expect(calls.filter((c) => c.url.endsWith("/ext/events"))).toEqual([]);
    expect(queued().length).toBe(1);

    // After the deadline the next alarm delivers and clears it.
    session.set("retry", { ...retry, at: Date.now() - 1 });
    await fake.api.alarms.onAlarm.fire({ name: "flush" });
    await settle();
    expect(calls.filter((c) => c.url.endsWith("/ext/events")).length).toBe(1);
    expect(session.has("retry")).toBe(false);
    expect(queued()).toEqual([]);
  });

  it("doubles the backoff up to five minutes", async () => {
    fakeFetch(() => undefined);
    fake.api.storage.session.data.set("retry", { at: 0, backoffMs: 4 * 60_000 });
    await startWorker();
    await fake.api.tabs.onActivated.fire({ tabId: 10, windowId: 1 });
    fake.api.storage.session.data.set("retry", { at: 0, backoffMs: 4 * 60_000 });
    await fake.api.alarms.onAlarm.fire({ name: "flush" });
    await settle();
    expect((fake.api.storage.session.data.get("retry") as { backoffMs: number }).backoffMs).toBe(5 * 60_000);
  });
});

describe("badge", () => {
  it("shows today's web time when switched on", async () => {
    fake.api.storage.local.data.set("badge", true);
    fakeFetch(appOn(47821, (path) => (path === "/stats/web" ? { body: { hosts: [{ host: "a", ms: 50 * 60_000 }, { host: "b", ms: 25 * 60_000 }] } } : undefined)));
    await startWorker();
    expect(fake.state.badge).toBe("1h");
  });

  it("stays empty when off", async () => {
    fakeFetch(appOn(47821, (path) => (path === "/stats/web" ? { body: { hosts: [{ host: "a", ms: 90 * 60_000 }] } } : undefined)));
    await startWorker();
    expect(fake.state.badge).toBe("");
  });
});
