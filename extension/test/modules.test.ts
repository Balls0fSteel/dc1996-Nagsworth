import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { appOn, fakeFetch, installFake, type Fake } from "./fake";

let fake: Fake;

beforeEach(() => {
  vi.resetModules();
  fake = installFake();
});

afterEach(() => {
  vi.unstubAllGlobals();
});

function ev(ts: number) {
  return { browser: "chrome", ts, kind: "tab_activated" as const, tab_id: ts, window_id: 1, url: null, title: null, incognito: false };
}

describe("queue", () => {
  it("keeps events in FIFO order and drops from the front", async () => {
    const q = await import("../src/queue");
    for (let i = 1; i <= 5; i++) await q.push(ev(i));
    await q.drop(2);
    expect((await q.load()).map((e) => e.ts)).toEqual([3, 4, 5]);
  });

  it("caps the queue at 5000 events, dropping the oldest", async () => {
    const q = await import("../src/queue");
    fake.api.storage.local.data.set("queue", Array.from({ length: q.MAX_QUEUE }, (_, i) => ev(i)));
    expect(await q.push(ev(99_999))).toBe(q.MAX_QUEUE);
    const all = await q.load();
    expect(all.length).toBe(5000);
    expect(all[0].ts).toBe(1);
    expect(all.at(-1)?.ts).toBe(99_999);
  });

  it("serializes concurrent pushes", async () => {
    const q = await import("../src/queue");
    await Promise.all(Array.from({ length: 50 }, (_, i) => q.push(ev(i))));
    expect((await q.load()).map((e) => e.ts)).toEqual(Array.from({ length: 50 }, (_, i) => i));
  });
});

describe("findPort", () => {
  it("probes the remembered port first", async () => {
    fake.api.storage.local.data.set("port", 47823);
    const calls = fakeFetch(appOn(47823));
    const { findPort } = await import("../src/api");
    expect(await findPort()).toBe(47823);
    expect(calls.map((c) => c.url)).toEqual(["http://127.0.0.1:47823/v1/health"]);
  });

  it("then probes the others in parallel and remembers the lowest that answers", async () => {
    fake.api.storage.local.data.set("port", 47821);
    let inFlight = 0;
    let maxInFlight = 0;
    const answer = appOn(47824);
    const calls = fakeFetch((port, path, call) => {
      inFlight++;
      maxInFlight = Math.max(maxInFlight, inFlight);
      queueMicrotask(() => inFlight--);
      if (port === 47822) return undefined;
      if (port === 47823 || port === 47824) return { body: { name: "nagsworth" } };
      return answer(port, path, call);
    });
    const { findPort } = await import("../src/api");
    expect(await findPort()).toBe(47823);
    expect(calls[0].url).toContain(":47821/");
    expect(calls.slice(1).map((c) => c.url).sort()).toEqual([
      "http://127.0.0.1:47822/v1/health",
      "http://127.0.0.1:47823/v1/health",
      "http://127.0.0.1:47824/v1/health",
    ]);
    expect(maxInFlight).toBe(3);
    expect(fake.api.storage.local.data.get("port")).toBe(47823);
  });

  it("ignores a server that is not Nagsworth", async () => {
    fakeFetch((port) => (port === 47821 ? { body: { name: "something else" } } : port === 47822 ? { body: { name: "nagsworth" } } : undefined));
    const { findPort } = await import("../src/api");
    expect(await findPort()).toBe(47822);
  });

  it("reports a hung app within about two probe timeouts", async () => {
    fake.api.storage.local.data.set("port", 47821);
    fakeFetch(() => "hang");
    const { findPort, PROBE_TIMEOUT_MS } = await import("../src/api");
    const t0 = Date.now();
    expect(await findPort()).toBeNull();
    const took = Date.now() - t0;
    expect(took).toBeGreaterThanOrEqual(PROBE_TIMEOUT_MS * 2 - 50);
    expect(took).toBeLessThan(PROBE_TIMEOUT_MS * 2 + 1000);
  });

  it("reuses the known port and re-probes only after a connection failure", async () => {
    fake.api.storage.local.data.set("token", "t");
    fake.api.storage.local.data.set("port", 47821);
    let up = 47821;
    const calls = fakeFetch((port, path, call) => appOn(up)(port, path, call));
    const { post } = await import("../src/api");
    await post("/ext/events", {});
    await post("/ext/events", {});
    expect(calls.filter((c) => c.url.endsWith("/health")).length).toBe(1);
    // The app restarts on another port.
    up = 47822;
    calls.length = 0;
    await post("/ext/events", {});
    expect(calls.map((c) => c.url)).toEqual([
      "http://127.0.0.1:47821/v1/ext/events",
      "http://127.0.0.1:47821/v1/health",
      "http://127.0.0.1:47822/v1/health",
      "http://127.0.0.1:47823/v1/health",
      "http://127.0.0.1:47824/v1/health",
      "http://127.0.0.1:47822/v1/ext/events",
    ]);
  });
});

describe("api", () => {
  it("forgets the token on 401", async () => {
    fake.api.storage.local.data.set("token", "old");
    fakeFetch(appOn(47821, () => ({ status: 401 })));
    const { post, NotPaired } = await import("../src/api");
    await expect(post("/ext/events", {})).rejects.toBeInstanceOf(NotPaired);
    expect(fake.api.storage.local.data.has("token")).toBe(false);
  });

  it("sends the bearer token", async () => {
    fake.api.storage.local.data.set("token", "secret");
    const calls = fakeFetch(appOn(47821));
    const { get } = await import("../src/api");
    await get("/state");
    expect(calls.at(-1)?.headers.Authorization).toBe("Bearer secret");
  });

  it("pairs with a code", async () => {
    fakeFetch(appOn(47821, (path, call) => (path === "/pair" && (call.body as { code: string }).code === "123456" ? { body: { token: "tok" } } : { status: 400 })));
    const { pair } = await import("../src/api");
    await expect(pair("000000")).rejects.toThrow("Wrong or expired code");
    await pair("123456");
    expect(fake.api.storage.local.data.get("token")).toBe("tok");
  });
});

describe("browserName", () => {
  const cases: [string, string, boolean?][] = [
    ["firefox", "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:159.0) Gecko/20100101 Firefox/159.0"],
    ["edge", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/156.0.0.0 Safari/537.36 Edg/156.0.0.0"],
    ["opera", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36 OPR/124.0.0.0"],
    ["vivaldi", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Vivaldi/7.6 Safari/537.36"],
    ["brave", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/156.0.0.0 Safari/537.36", true],
    ["chrome", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/156.0.0.0 Safari/537.36"],
  ];
  for (const [family, ua, brave] of cases) {
    it(`detects ${family}`, async () => {
      vi.stubGlobal("navigator", brave ? { userAgent: ua, brave: {} } : { userAgent: ua });
      const { browserName } = await import("../src/browser");
      expect(browserName()).toBe(family);
    });
  }
});
