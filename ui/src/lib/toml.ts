// Small line-based edits of the config TOML, so a settings switch can change
// one key without reformatting the file or losing its comments. Pure
// functions; the result goes back through `PUT /rules`.
//
// Limits: only plain `key = value` lines inside a `[table]` header are seen.
// Dotted keys (`tracking.x = 1`), inline tables (`tracking = { ... }`) and
// `[` lines inside multi-line strings are not understood.

/** Any table or array-of-tables header, e.g. `[general]`, `[[rules]]`. */
const ANY_HEADER = /^\s*\[\[?\s*[A-Za-z0-9_.\-"' ]+\s*\]\]?\s*(#.*)?$/;
const BLANK_OR_COMMENT = /^\s*(#.*)?$/;

function tableHeader(name: string): RegExp {
  return new RegExp(`^\\s*\\[\\s*${name}\\s*\\]\\s*(#.*)?$`);
}

/** `key = value  # comment`: groups are prefix, value, trailing space + comment. */
function keyLine(key: string): RegExp {
  return new RegExp(`^(\\s*${key}\\s*=\\s*)([^#]*?)(\\s*(?:#.*)?)$`);
}

/** Line range of `[name]`: header index and the first line after the table; null if missing. */
function findTable(lines: string[], name: string): { header: number; end: number } | null {
  const re = tableHeader(name);
  const header = lines.findIndex((l) => re.test(l));
  if (header < 0) return null;
  let end = header + 1;
  while (end < lines.length && !ANY_HEADER.test(lines[end])) end++;
  return { header, end };
}

/** Lines split on "\n" with any "\r" kept apart, so regexes see clean text. */
function splitLines(src: string): { lines: string[]; eol: string } {
  const eol = src.includes("\r\n") ? "\r\n" : "\n";
  return { lines: src.split("\n").map((l) => l.replace(/\r$/, "")), eol };
}

/** The raw value text of `key` in `[table]`, or null when the key or table is missing. */
export function getTableValue(src: string, table: string, key: string): string | null {
  const { lines } = splitLines(src);
  const t = findTable(lines, table);
  if (!t) return null;
  const re = keyLine(key);
  for (let i = t.header + 1; i < t.end; i++) {
    const m = re.exec(lines[i]);
    if (m) return m[2].trim();
  }
  return null;
}

/**
 * Sets `key = value` (value is literal TOML text) in `[table]`:
 * - an existing key line keeps its indentation and trailing comment;
 * - a missing key goes after the table's last non-blank, non-comment line
 *   (right after the header when the table is empty);
 * - a missing table is created after the `[after]` table's last key line,
 *   else before the first table header, else at the end of the file.
 * Line endings follow the file (CRLF if it has any).
 */
export function setTableValue(
  src: string,
  table: string,
  key: string,
  value: string,
  after = "general",
): string {
  const { lines, eol } = splitLines(src);
  const t = findTable(lines, table);
  if (t) {
    const re = keyLine(key);
    for (let i = t.header + 1; i < t.end; i++) {
      if (re.test(lines[i])) {
        lines[i] = lines[i].replace(re, (_, pre: string, _v: string, post: string) => pre + value + post);
        return lines.join(eol);
      }
    }
    let at = t.header;
    for (let i = t.header + 1; i < t.end; i++) {
      if (!BLANK_OR_COMMENT.test(lines[i])) at = i;
    }
    lines.splice(at + 1, 0, `${key} = ${value}`);
    return lines.join(eol);
  }

  const block = [`[${table}]`, `${key} = ${value}`];
  const prev = findTable(lines, after);
  if (prev) {
    let at = prev.header;
    for (let i = prev.header + 1; i < prev.end; i++) {
      if (!BLANK_OR_COMMENT.test(lines[i])) at = i;
    }
    lines.splice(at + 1, 0, "", ...block);
    return lines.join(eol);
  }
  const first = lines.findIndex((l) => ANY_HEADER.test(l));
  if (first >= 0) {
    // Keep a comment block that sits right on top of the first header with it.
    let at = first;
    while (at > 0 && /^\s*#/.test(lines[at - 1])) at--;
    lines.splice(at, 0, ...block, "");
    return lines.join(eol);
  }
  // No tables at all: append (root keys must stay above any header).
  while (lines.length > 0 && lines[lines.length - 1].trim() === "") lines.pop();
  if (lines.length > 0) lines.push("");
  lines.push(...block, "");
  return lines.join(eol);
}

// ---- [exercises] ----

export interface ExerciseSettings {
  enabled: boolean;
  equipment: string[];
  exclude: string[];
  walks: boolean;
}

/** A TOML array of bare words or quoted strings, e.g. `["band", "floor"]`. */
function parseStringArray(v: string | null): string[] | null {
  if (v === null) return null;
  const m = /^\[(.*)\]$/s.exec(v.trim());
  if (!m) return null;
  return m[1]
    .split(",")
    .map((s) => s.trim().replace(/^["']|["']$/g, ""))
    .filter((s) => s.length > 0);
}

function tomlStringArray(items: readonly string[]): string {
  return `[${items.map((s) => JSON.stringify(s)).join(", ")}]`;
}

/** Reads `[exercises]` with the service's defaults (on, nothing, nothing, walks on). */
export function readExercises(src: string): ExerciseSettings {
  const enabled = getTableValue(src, "exercises", "enabled");
  const walks = getTableValue(src, "exercises", "walks");
  return {
    enabled: enabled === null ? true : enabled !== "false",
    equipment: parseStringArray(getTableValue(src, "exercises", "equipment")) ?? [],
    exclude: parseStringArray(getTableValue(src, "exercises", "exclude")) ?? [],
    walks: walks === null ? true : walks !== "false",
  };
}

/** Writes the given `[exercises]` keys; the table is created after `[api]` when missing. */
export function withExercises(src: string, s: Partial<ExerciseSettings>): string {
  let out = src;
  const set = (key: string, value: string) => {
    out = setTableValue(out, "exercises", key, value, "api");
  };
  if (s.enabled !== undefined) set("enabled", String(s.enabled));
  if (s.equipment !== undefined) set("equipment", tomlStringArray(s.equipment));
  if (s.exclude !== undefined) set("exclude", tomlStringArray(s.exclude));
  if (s.walks !== undefined) set("walks", String(s.walks));
  return out;
}

// ---- [tracking] hands-on settings ----

export const INPUT_WINDOW_MIN = 1;
export const INPUT_WINDOW_MAX = 600;

export interface HandsOnSettings {
  hands_on_games: boolean;
  input_window_s: number;
}

/** Reads the hands-on keys of `[tracking]`, with the service's defaults (true, 5). */
export function readHandsOn(src: string): HandsOnSettings {
  const games = getTableValue(src, "tracking", "hands_on_games");
  const win = getTableValue(src, "tracking", "input_window_s");
  const n = win !== null && /^\d+$/.test(win) ? Number(win) : NaN;
  return {
    hands_on_games: games === null ? true : games !== "false",
    input_window_s: Number.isFinite(n) ? n : 5,
  };
}

/** Writes the given hands-on keys into `[tracking]`; `input_window_s` is rounded and clamped. */
export function withHandsOn(src: string, s: Partial<HandsOnSettings>): string {
  let out = src;
  if (s.hands_on_games !== undefined) {
    out = setTableValue(out, "tracking", "hands_on_games", String(s.hands_on_games));
  }
  if (s.input_window_s !== undefined) {
    const n = Math.min(INPUT_WINDOW_MAX, Math.max(INPUT_WINDOW_MIN, Math.round(s.input_window_s)));
    out = setTableValue(out, "tracking", "input_window_s", String(n));
  }
  return out;
}
