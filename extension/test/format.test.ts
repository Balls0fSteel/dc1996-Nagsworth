import { describe, expect, it } from "vitest";
import {
  ago,
  badgeText,
  browserFamily,
  canAct,
  counting,
  duration,
  hostOf,
  privateWindowHint,
  ruleFraction,
  ruleProgress,
  type RuleSnapshot,
} from "../src/format";

describe("format", () => {
  it("formats durations", () => {
    expect(duration(0)).toBe("<1m");
    expect(duration(12 * 60_000 + 59_000)).toBe("12m");
    expect(duration(72 * 60_000)).toBe("1h 12m");
    expect(duration(62 * 60_000)).toBe("1h 02m");
  });

  it("keeps badge text short", () => {
    expect(badgeText(30_000)).toBe("");
    expect(badgeText(45 * 60_000)).toBe("45m");
    expect(badgeText(135 * 60_000)).toBe("2h");
  });

  it("formats ages and hosts", () => {
    expect(ago(1000, 13_000)).toBe("12 s ago");
    expect(ago(0, 5 * 60_000)).toBe("5 min ago");
    expect(hostOf("https://www.example.org/a?b")).toBe("www.example.org");
    expect(hostOf("about:blank")).toBe("about");
    expect(hostOf(null)).toBeNull();
  });

  it("maps exe names to browser families like the app", () => {
    expect(browserFamily("msedge.exe")).toBe("edge");
    expect(browserFamily("FIREFOX.EXE")).toBe("firefox");
    expect(browserFamily("librewolf")).toBe("firefox");
    expect(browserFamily("opera_gx.exe")).toBe("opera");
    expect(browserFamily("Code.exe")).toBeNull();
  });

  it("says whether the app counts this browser", () => {
    const edge = { kind: "active" as const, app: { exe_name: "msedge.exe" } };
    expect(counting({ activity: edge }, "edge")).toBe("counting");
    expect(counting({ activity: edge }, "firefox")).toBe("not_in_front");
    expect(counting({ activity: { kind: "active", app: { exe_name: "Code.exe" } } }, "edge")).toBe("not_in_front");
    expect(counting({ activity: { ...edge, kind: "idle" } }, "edge")).toBe("idle");
    expect(counting({ activity: edge, tracking_paused: true }, "edge")).toBe("paused");
    expect(counting({}, "edge")).toBe("unknown");
  });

  it("describes rule progress", () => {
    const now = 1_000_000;
    const base: RuleSnapshot = { id: "r", name: "Rest break", trigger: "interval", enabled: true, state: { state: "idle" }, accum_ms: 48 * 60_000, target_ms: 60 * 60_000, snoozes_left: 2 };
    expect(ruleProgress(base, now)).toBe("in 12m");
    expect(ruleFraction(base)).toBeCloseTo(0.8);
    const limit = { ...base, trigger: "daily_total", accum_ms: 62 * 60_000, target_ms: 120 * 60_000 };
    expect(ruleProgress(limit, now)).toBe("1h 02m of 2h 00m");
    expect(ruleProgress({ ...base, state: { state: "snoozed", until: now + 5 * 60_000 } }, now)).toBe("Snoozed, 5m left");
    expect(ruleProgress({ ...limit, state: { state: "satisfied" } }, now)).toBe("Over the limit");
    expect(ruleProgress({ ...base, trigger: "schedule", target_ms: 0 }, now)).toBe("Schedule");
  });

  it("offers Snooze and Skip only for a due or nagging rule", () => {
    const r: RuleSnapshot = { id: "r", name: "x", trigger: "interval", enabled: true, state: { state: "active" }, snoozes_left: 1 };
    expect(canAct(r)).toEqual({ snooze: true, skip: true });
    expect(canAct({ ...r, snoozes_left: 0 })).toEqual({ snooze: false, skip: true });
    expect(canAct({ ...r, state: { state: "pending" } })).toEqual({ snooze: true, skip: true });
    expect(canAct({ ...r, state: { state: "idle" } })).toEqual({ snooze: false, skip: false });
    expect(canAct({ ...r, enabled: false })).toEqual({ snooze: false, skip: false });
  });

  it("names where to allow private windows", () => {
    expect(privateWindowHint("firefox")).toContain("about:addons");
    expect(privateWindowHint("edge")).toContain("chrome://extensions");
  });
});
