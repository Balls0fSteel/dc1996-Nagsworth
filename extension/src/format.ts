// Pure helpers shared by the popup and the badge; no WebExtension API here.

/** "1h 12m", "12m", "<1m". */
export function duration(ms: number): string {
  const min = Math.floor(Math.max(0, ms) / 60_000);
  if (min < 1) return "<1m";
  const h = Math.floor(min / 60);
  const m = min % 60;
  if (h === 0) return `${m}m`;
  return `${h}h ${String(m).padStart(2, "0")}m`;
}

/** Toolbar badge text: at most four characters ("1h", "12m", "" for none). */
export function badgeText(ms: number): string {
  const min = Math.floor(Math.max(0, ms) / 60_000);
  if (min < 1) return "";
  if (min < 60) return `${min}m`;
  return `${Math.floor(min / 60)}h`;
}

/** "12 s ago", "5 min ago", "2 h ago". */
export function ago(ts: number, now: number): string {
  const s = Math.max(0, Math.round((now - ts) / 1000));
  if (s < 60) return `${s} s ago`;
  if (s < 3600) return `${Math.floor(s / 60)} min ago`;
  return `${Math.floor(s / 3600)} h ago`;
}

export function hostOf(url: string | null | undefined): string | null {
  if (!url) return null;
  try {
    const u = new URL(url);
    return u.host || u.protocol.replace(/:$/, "");
  } catch {
    return null;
  }
}

// ---- Shapes from the app's API (crates/ns-api/src/routes.rs). Every field
// may be missing on another app version, so readers treat them as optional.

export interface AppRef {
  exe_name?: string;
}

export interface Activity {
  kind?: "active" | "idle" | "locked" | "suspended" | "paused" | null;
  app?: AppRef | null;
}

export type NagState =
  | { state: "idle" }
  | { state: "pending" }
  | { state: "active"; level?: string; since?: number }
  | { state: "snoozed"; until: number }
  | { state: "in_break"; until: number }
  | { state: "satisfied"; since?: number };

export interface RuleSnapshot {
  id: string;
  name: string;
  trigger: string;
  enabled: boolean;
  state?: NagState;
  accum_ms?: number;
  target_ms?: number;
  snoozes_left?: number;
  message?: string;
}

export interface AppState {
  now?: number;
  today?: string;
  activity?: Activity;
  tracking_paused?: boolean;
  engine?: { rules?: RuleSnapshot[] };
}

export interface HostUsage {
  host: string;
  ms: number;
}

/** Browser family for an exe, as the app's `browser_family` derives it. */
export function browserFamily(exe: string | undefined): string | null {
  if (!exe) return null;
  const stem = exe.toLowerCase().replace(/\.exe$/, "");
  const families: Record<string, string[]> = {
    chrome: ["chrome", "chromium", "google-chrome", "chromium-browser"],
    edge: ["msedge", "microsoft-edge"],
    firefox: ["firefox", "firefox-bin", "librewolf", "waterfox", "floorp", "zen"],
    brave: ["brave", "brave-browser"],
    opera: ["opera", "opera_gx"],
    vivaldi: ["vivaldi", "vivaldi-bin"],
    arc: ["arc"],
  };
  for (const [family, stems] of Object.entries(families)) if (stems.includes(stem)) return family;
  return null;
}

export type Counting = "counting" | "not_in_front" | "idle" | "locked" | "paused" | "unknown";

/** Whether the app is counting this browser's tab right now. */
export function counting(state: AppState, browser: string): Counting {
  if (state.tracking_paused || state.activity?.kind === "paused") return "paused";
  const kind = state.activity?.kind;
  if (kind === "idle") return "idle";
  if (kind === "locked" || kind === "suspended") return "locked";
  if (kind !== "active") return "unknown";
  const family = browserFamily(state.activity?.app?.exe_name);
  return family === browser ? "counting" : "not_in_front";
}

export const COUNTING_TEXT: Record<Counting, string> = {
  counting: "Counting",
  not_in_front: "Browser not in front",
  idle: "Idle",
  locked: "Computer locked",
  paused: "Tracking paused",
  unknown: "Not counting",
};

/** One line of progress for a rule, e.g. "in 12m" or "1h 02m of 2h 00m". */
export function ruleProgress(r: RuleSnapshot, now: number): string {
  const s = r.state ?? { state: "idle" };
  switch (s.state) {
    case "active":
      return "Nagging now";
    case "pending":
      return "Due";
    case "snoozed":
      return `Snoozed, ${duration(s.until - now)} left`;
    case "in_break":
      return `On break, ${duration(s.until - now)} left`;
    case "satisfied":
      return r.trigger === "daily_total" ? "Over the limit" : "Skipped";
  }
  const accum = r.accum_ms ?? 0;
  const target = r.target_ms ?? 0;
  if (target <= 0) return r.trigger === "schedule" ? "Schedule" : "";
  if (r.trigger === "daily_total") return `${duration(accum)} of ${duration(target)}`;
  return `in ${duration(target - accum)}`;
}

/** Fraction of the target reached, 0..1. */
export function ruleFraction(r: RuleSnapshot): number {
  const target = r.target_ms ?? 0;
  return target > 0 ? Math.min(1, Math.max(0, (r.accum_ms ?? 0) / target)) : 0;
}

/** Snooze and Skip apply to a rule that is due or nagging. */
export function canAct(r: RuleSnapshot): { snooze: boolean; skip: boolean } {
  const st = r.state?.state;
  const due = r.enabled && (st === "pending" || st === "active");
  return { snooze: due && (r.snoozes_left ?? 0) > 0, skip: due };
}
