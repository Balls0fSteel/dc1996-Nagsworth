export function duration(ms: number): string {
  const totalMin = Math.floor(ms / 60_000);
  const h = Math.floor(totalMin / 60);
  const m = totalMin % 60;
  if (h > 0) return `${h}h ${m.toString().padStart(2, "0")}m`;
  if (totalMin > 0) return `${m}m`;
  return `${Math.floor(ms / 1000)}s`;
}

export function clock(ms: number): string {
  const s = Math.max(0, Math.ceil(ms / 1000));
  const m = Math.floor(s / 60);
  return `${m}:${(s % 60).toString().padStart(2, "0")}`;
}

export function time(ts: number): string {
  return new Date(ts).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
}

export function dateTime(ts: number): string {
  return new Date(ts).toLocaleString([], {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

/** YYYY-MM-DD shifted by `days`. */
export function addDays(day: string, days: number): string {
  const d = new Date(`${day}T12:00:00Z`);
  d.setUTCDate(d.getUTCDate() + days);
  return d.toISOString().slice(0, 10);
}

export const categoryColor: Record<string, string> = {
  Work: "var(--cat-work)",
  Game: "var(--cat-game)",
  Web: "var(--cat-web)",
  Other: "var(--cat-other)",
};

export const kindColor: Record<string, string> = {
  idle: "var(--seg-idle)",
  locked: "var(--seg-locked)",
  suspended: "var(--seg-suspended)",
  paused: "var(--seg-paused)",
};

/** "just now", "3 min ago", "2 h ago", "yesterday", "3 days ago", then a date. */
export function relative(ts: number, now: number = Date.now()): string {
  const diff = now - ts;
  if (diff < 0) return "just now";
  const min = Math.floor(diff / 60_000);
  if (min < 1) return "just now";
  if (min < 60) return `${min} min ago`;
  const h = Math.floor(min / 60);
  if (h < 24) return `${h} h ago`;
  const a = new Date(ts);
  const b = new Date(now);
  const days = Math.round(
    (Date.UTC(b.getFullYear(), b.getMonth(), b.getDate()) -
      Date.UTC(a.getFullYear(), a.getMonth(), a.getDate())) /
      86_400_000,
  );
  if (days <= 1) return "yesterday";
  if (days < 30) return `${days} days ago`;
  return a.toLocaleDateString([], { day: "numeric", month: "short", year: "numeric" });
}

/** Share of `total` as a whole percentage, "12%"; "0%" when total is 0. */
export function percent(part: number, total: number): string {
  if (!(total > 0)) return "0%";
  const p = (part / total) * 100;
  if (p > 0 && p < 1) return "<1%";
  return `${Math.round(p)}%`;
}

/** Compact hours for axes and tiles: "1.5h", "12h", "45m", "0m". */
export function hoursShort(ms: number): string {
  const min = Math.round(ms / 60_000);
  if (min < 60) return `${min}m`;
  const h = min / 60;
  return `${h >= 10 ? Math.round(h) : Math.round(h * 10) / 10}h`;
}

/** "Today", "Yesterday" or "Mon 3 Nov" for a YYYY-MM-DD local day. */
export function localDayLabel(day: string, today: string): string {
  if (day === today) return "Today";
  if (day === addDays(today, -1)) return "Yesterday";
  const d = new Date(`${day}T12:00:00Z`);
  const wd = d.toLocaleDateString("en-GB", { weekday: "short", timeZone: "UTC" });
  const dm = d.toLocaleDateString("en-GB", { day: "numeric", month: "short", timeZone: "UTC" });
  return `${wd} ${dm}`;
}
