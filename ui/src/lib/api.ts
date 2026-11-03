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
}

export type NagState =
  | { state: "idle" }
  | { state: "pending" }
  | { state: "active"; level: NagLevel; since: number }
  | { state: "snoozed"; until: number }
  | { state: "in_break"; until: number }
  | { state: "satisfied"; since: number };

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
  engine: { at: number; reading_mode: boolean; rules: RuleSnapshot[] };
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
}

export interface CategoryUsage {
  category_id: number;
  name: string;
  kind: CategoryKind;
  color: string | null;
  ms: number;
}

export interface DailyUsage {
  day: string;
  category: string;
  kind: CategoryKind;
  color: string | null;
  ms: number;
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
  rules: () => request<{ toml: string; rules: unknown[]; error?: string }>("GET", "/rules"),
  saveRules: (toml: string) => request<{ toml: string }>("PUT", "/rules", toml, true),
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
