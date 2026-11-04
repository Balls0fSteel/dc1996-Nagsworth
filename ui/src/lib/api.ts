// Client for the local HTTP API. Inside the app the base URL and token come
// from the `get_api_info` command; in a plain browser (vite dev) they can be
// passed as `?port=47821&token=...` and are remembered in localStorage.

import { invoke, isTauri } from "@tauri-apps/api/core";

export type CategoryKind = "Work" | "Game" | "Web" | "Other";
export type SegmentKind = "active" | "idle" | "locked" | "suspended" | "paused";
export type NagLevel = "toast" | "overlay" | "lock";

export interface AppRef {
  id: number;
  exe_name: string;
  category: CategoryKind;
  is_game: boolean;
}

export interface Activity {
  ts: number;
  kind: SegmentKind | null;
  app: AppRef | null;
  title: string | null;
  idle_ms: number;
  is_fullscreen: boolean;
  /** Input within the last `input_window_s` (hands-on time). */
  hands_on: boolean;
}

export type NagState =
  | { state: "idle" }
  | { state: "pending" }
  | { state: "active"; level: NagLevel; since: number }
  | { state: "snoozed"; until: number }
  | { state: "in_break"; until: number }
  | { state: "satisfied"; since: number };

/** One exercise of a break routine (`RoutineStep` in ns-core). */
export interface RoutineStep {
  id: string;
  name: string;
  seconds: number;
  reps: string;
  cue: string;
  lines: string[];
}

/** What to do during a break (docs/EXERCISES.md). */
export interface Routine {
  steps: RoutineStep[];
  note: string | null;
}

export interface RuleSnapshot {
  id: string;
  name: string;
  trigger: "interval" | "daily_total" | "continuous" | "schedule";
  enabled: boolean;
  state: NagState;
  accum_ms: number;
  target_ms: number;
  rest_ms: number;
  break_ms: number;
  snoozes_left: number;
  message: string;
  /** The effective idle policy (the default resolved). */
  idle_policy: IdlePolicy;
  /** The exercises of the open nag or the running break. */
  routine: Routine | null;
}

export interface ExtClient {
  browser: string;
  version: string | null;
  last_seen: number;
}

export interface StateResponse {
  now: number;
  today: string;
  activity: Activity;
  tracking_paused: boolean;
  engine: { at: number; reading_mode: boolean; hands_on_games: boolean; rules: RuleSnapshot[] };
  extensions: [string, ExtClient][];
}

export interface AppUsage {
  app_id: number;
  exe_name: string;
  display_name: string | null;
  category: string;
  category_kind: CategoryKind;
  is_game: boolean;
  ms: number;
  input_ms: number;
}

export interface CategoryUsage {
  category_id: number;
  name: string;
  kind: CategoryKind;
  color: string | null;
  ms: number;
  input_ms: number;
}

/** Started breaks and how they ended (`/stats/breaks`). */
export interface BreakCounts {
  started: number;
  /** Ran to the end. */
  completed: number;
  ended_early: number;
  with_exercises: number;
  with_exercises_completed: number;
}

export interface BreakDay extends BreakCounts {
  day: string;
}

export interface DailyUsage {
  day: string;
  category: string;
  kind: CategoryKind;
  color: string | null;
  ms: number;
  input_ms: number;
}

export interface TimelineEntry {
  id: number;
  start: number;
  end: number;
  kind: SegmentKind;
  app_id: number | null;
  exe_name: string | null;
  title: string | null;
  category_kind: CategoryKind | null;
  /** Hands-on milliseconds within the segment. */
  input_ms: number;
}

export interface HostUsage {
  host: string;
  ms: number;
}

export interface AppRow {
  id: number;
  exe_name: string;
  exe_path: string | null;
  display_name: string | null;
  category_id: number;
  category: string;
  category_kind: CategoryKind;
  category_source: "auto" | "user";
  is_game: boolean;
  first_seen: number;
  last_seen: number;
}

export interface Category {
  id: number;
  name: string;
  kind: CategoryKind;
  color: string | null;
}

export interface NagLogEntry {
  id: number;
  rule_id: string;
  kind: string;
  level: string | null;
  reason: string | null;
  at: number;
  /** The suggested exercises of a raise or break start. */
  routine: Routine | null;
}

export type ExerciseTag =
  | "eyes"
  | "neck"
  | "shoulders"
  | "upper_back"
  | "wrists"
  | "lower_back"
  | "hips"
  | "legs"
  | "walk"
  | "cardio"
  | "breathing"
  | "habits";

export type Equipment = "band" | "stairs" | "floor";

/** An item of the catalogue (`Exercise` in ns-rules). */
export interface Exercise {
  id: string;
  name: string;
  tags: ExerciseTag[];
  posture: "seated" | "standing" | "floor";
  seconds: number;
  reps: string;
  equipment: Equipment[];
  cue: string;
  steps: string[];
  evidence: "A" | "B" | "C";
  why: string;
  enabled: boolean;
}

/** `[exercises]` in the config file. */
export interface ExercisesConfig {
  enabled: boolean;
  equipment: Equipment[];
  exclude: string[];
  walks: boolean;
  habits: boolean;
  step_sound: boolean;
}

export interface ExercisesResponse {
  exercises: Exercise[];
  config: ExercisesConfig;
}

/** A humantime duration as the config writes it, e.g. "50m", "1h 30m", "0s". */
export type HumanDuration = string;

/** `Trigger` in `crates/ns-rules/src/rule.rs`: internally tagged by `kind`. */
export type TriggerDef =
  | { kind: "interval"; every: HumanDuration; break_for: HumanDuration }
  | { kind: "daily_total"; limit: HumanDuration; renag_every: HumanDuration }
  | { kind: "continuous"; limit: HumanDuration; reset_after: HumanDuration }
  /** Windows are "HH:MM-HH:MM"; the end may be before the start (wraps midnight). */
  | { kind: "schedule"; windows: string[] };

/** `Scope`: the string "any", `{ category }` or `{ app }` (an exe name). */
export type ScopeDef = "any" | { category: CategoryKind } | { app: string };

/** `input` counts only hands-on time (seconds after an input). */
export type IdlePolicy = "pause" | "ignore" | "input";

export interface EscalationStep {
  after: HumanDuration;
  level: NagLevel;
}

export interface SnoozeDef {
  duration: HumanDuration;
  max_per_day: number;
}

/** A `[[rules]]` table as `GET /rules` returns it (serde of `Rule`). */
export interface RuleDef {
  id: string;
  name: string | null;
  enabled: boolean;
  scope: ScopeDef;
  trigger: TriggerDef;
  /** `null` means the default: for the Game category `input` when `[tracking] hands_on_games` is on, else `ignore`; `pause` otherwise. */
  idle_policy: IdlePolicy | null;
  escalation: EscalationStep[];
  /** `null` means the default: 5m, 3 per day. */
  snooze: SnoozeDef | null;
  message: string | null;
  /** `false`: no exercises; `true`: the full planner; tags: only those regions. */
  exercises: boolean | { tags: ExerciseTag[] };
}

export interface RulesResponse {
  toml: string;
  rules: RuleDef[];
  /** Set when the file does not parse; `rules` is then empty. */
  error?: string;
}

export interface Health {
  ok: boolean;
  name: string;
  version: string;
}

/** Per-day totals by segment kind (`KindTotal` in ns-storage). */
export interface KindTotal {
  day: string;
  kind: SegmentKind;
  ms: number;
  input_ms: number;
}

export interface ApiInfo {
  base_url: string;
  token: string;
}

let info: Promise<ApiInfo> | null = null;

async function loadInfo(): Promise<ApiInfo> {
  if (isTauri()) {
    return invoke<ApiInfo>("get_api_info");
  }
  const q = new URLSearchParams(location.search);
  const port = q.get("port") ?? localStorage.getItem("an.port") ?? "47821";
  const token = q.get("token") ?? localStorage.getItem("an.token") ?? "";
  localStorage.setItem("an.port", port);
  localStorage.setItem("an.token", token);
  return { base_url: `http://127.0.0.1:${port}/v1`, token };
}

export function apiInfo(): Promise<ApiInfo> {
  info ??= loadInfo();
  return info;
}

export class ApiError extends Error {
  constructor(
    public status: number,
    message: string,
  ) {
    super(message);
  }
}

async function request<T>(method: string, path: string, body?: unknown, raw = false): Promise<T> {
  const { base_url, token } = await apiInfo();
  const headers: Record<string, string> = { Authorization: `Bearer ${token}` };
  let payload: BodyInit | undefined;
  if (body !== undefined) {
    if (raw) {
      payload = body as string;
      headers["Content-Type"] = "text/plain";
    } else {
      payload = JSON.stringify(body);
      headers["Content-Type"] = "application/json";
    }
  }
  const res = await fetch(base_url + path, { method, headers, body: payload });
  const text = await res.text();
  const data = text ? JSON.parse(text) : null;
  if (!res.ok) {
    throw new ApiError(res.status, data?.error ?? text ?? res.statusText);
  }
  return data as T;
}

const range = (from?: string, to?: string) => {
  const q = new URLSearchParams();
  if (from) q.set("from", from);
  if (to) q.set("to", to);
  const s = q.toString();
  return s ? `?${s}` : "";
};

export const api = {
  state: () => request<StateResponse>("GET", "/state"),
  appsUsage: (from?: string, to?: string) =>
    request<{ apps: AppUsage[] }>("GET", `/stats/apps${range(from, to)}`).then((r) => r.apps),
  categoriesUsage: (from?: string, to?: string) =>
    request<{ categories: CategoryUsage[] }>("GET", `/stats/categories${range(from, to)}`).then(
      (r) => r.categories,
    ),
  breaks: (from?: string, to?: string) =>
    request<{ days: BreakDay[]; total: BreakCounts }>("GET", `/stats/breaks${range(from, to)}`),
  daily: (from?: string, to?: string) =>
    request<{ days: DailyUsage[] }>("GET", `/stats/daily${range(from, to)}`).then((r) => r.days),
  timeline: (day?: string) =>
    request<{ segments: TimelineEntry[] }>("GET", `/stats/timeline${day ? `?day=${day}` : ""}`).then(
      (r) => r.segments,
    ),
  web: (from?: string, to?: string) =>
    request<{ hosts: HostUsage[] }>("GET", `/stats/web${range(from, to)}`).then((r) => r.hosts),
  apps: () => request<{ apps: AppRow[] }>("GET", "/apps").then((r) => r.apps),
  updateApp: (
    id: number,
    patch: { category_id?: number; is_game?: boolean; display_name?: string | null },
  ) => request<{ app: AppRow }>("PATCH", `/apps/${id}`, patch).then((r) => r.app),
  categories: () => request<{ categories: Category[] }>("GET", "/categories").then((r) => r.categories),
  createCategory: (name: string, kind: CategoryKind, color?: string) =>
    request<{ category: Category }>("POST", "/categories", { name, kind, color }),
  kinds: (from?: string, to?: string) =>
    request<{ from: string; to: string; kinds: KindTotal[] }>(
      "GET",
      `/stats/kinds${range(from, to)}`,
    ).then((r) => r.kinds),
  health: () => request<Health>("GET", "/health"),
  rules: () => request<RulesResponse>("GET", "/rules"),
  exercises: () => request<ExercisesResponse>("GET", "/exercises"),
  saveRules: (toml: string) => request<RulesResponse>("PUT", "/rules", toml, true),
  nags: (limit = 50) =>
    request<{ nags: NagLogEntry[] }>("GET", `/nags?limit=${limit}`).then((r) => r.nags),
  startPairing: () => request<{ code: string; expires_at: number }>("POST", "/pair/start"),
  snooze: (rule_id: string) => request("POST", "/control/snooze", { rule_id }),
  skip: (rule_id: string) => request("POST", "/control/skip", { rule_id }),
  startBreak: (rule_id?: string) => request("POST", "/control/break/start", { rule_id }),
  pause: (minutes?: number) => request("POST", "/control/pause", { minutes }),
  resume: () => request("POST", "/control/resume"),
  readingMode: (on: boolean) => request("POST", "/control/reading-mode", { on }),
  backup: () => request<{ path: string }>("POST", "/backup"),
};

/** Downloads `/export` as a file through a blob link (the route needs the token). */
export async function downloadExport(format: "csv" | "json", from: string, to: string) {
  const { base_url, token } = await apiInfo();
  const res = await fetch(`${base_url}/export?format=${format}&from=${from}&to=${to}`, {
    headers: { Authorization: `Bearer ${token}` },
  });
  if (!res.ok) throw new ApiError(res.status, await res.text());
  const url = URL.createObjectURL(await res.blob());
  const a = document.createElement("a");
  a.href = url;
  a.download = `activity-${from}-${to}.${format}`;
  a.click();
  setTimeout(() => URL.revokeObjectURL(url), 10_000);
}

/** Autostart lives in the app, not the HTTP API; `null` outside the app. */
export async function getAutostart(): Promise<boolean | null> {
  return isTauri() ? invoke<boolean>("get_autostart") : null;
}

export async function setAutostart(enabled: boolean): Promise<void> {
  await invoke("set_autostart", { enabled });
}

// ---- Updates (src-tauri/src/update.rs); app only, like autostart ----

export interface UpdateStatus {
  current: string;
  /** `false` for a build without an update check. */
  enabled: boolean;
  auto_check: boolean;
  beta: boolean;
  /** Installed by the setup: the app can download and install the update itself. */
  can_install: boolean;
  checking: boolean;
  available: { version: string; url: string } | null;
  /** The version the banner shows, until "Later". */
  banner: string | null;
  download: { got: number; total: number | null } | null;
  /** The version whose setup is downloaded and checked. */
  ready: string | null;
  error: string | null;
  last_check: number | null;
}

export const UPDATE_EVENT = "update-status";

export async function updateStatus(): Promise<UpdateStatus | null> {
  return isTauri() ? invoke<UpdateStatus>("update_status") : null;
}

export const updateApi = {
  check: () => invoke<void>("update_check"),
  download: () => invoke<void>("update_download"),
  cancel: () => invoke<void>("update_cancel"),
  install: () => invoke<void>("update_install"),
  dismiss: () => invoke<void>("update_dismiss"),
  setAuto: (on: boolean) => invoke<void>("update_set_auto", { on }),
  setBeta: (on: boolean) => invoke<void>("update_set_beta", { on }),
  openReleasePage: () => invoke<void>("update_open_release_page"),
};
