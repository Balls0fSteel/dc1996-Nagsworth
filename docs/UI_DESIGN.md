# Nagsworth UI design pass

Status (2026-11-04_093504_1793781304): implemented as written below, with
these differences: the toast window is sized to its content through a
`resize_toast` command instead of a fixed height, and shows up to three
cards; the Today page fetches the rule definitions so it can show scope
chips and schedule windows; History remembers its range in
`localStorage`; the Rust side changed only in `windows.rs`,
`commands.rs` and `nag_bridge.rs` (see CHANGELOG.md).

Goal: make the dashboard, nag windows and pairing page look finished and feel
good to use, without changing the API or the Rust side except where a window
size needs it. Svelte 5 + TypeScript, no UI framework, no icon font; icons are
inline SVG paths in `ui/src/lib/icons.ts`.

## 1. Design system (`ui/src/app.css`, `ui/src/lib/ui/`)

Tokens (light and dark via `prefers-color-scheme`):

- Surfaces: `--bg` (page), `--panel` (cards), `--panel-2` (inset areas such
  as table headers and the timeline track), `--border`, `--border-strong`.
- Text: `--text`, `--text-2` (secondary), `--muted` (tertiary).
- Brand: `--accent` indigo `#4f46e5` (the icon's blue), `--accent-soft`
  (tinted background), `--accent-text`; `--danger`, `--danger-soft`, `--ok`,
  `--ok-soft`, `--warn`, `--warn-soft`.
- Categories: Work blue, Game coral red (the icon's badge), Web green, Other
  grey; segment kinds idle, locked, suspended, paused.
- Shape and depth: `--radius-sm: 6px`, `--radius: 10px`, `--radius-lg: 14px`,
  `--shadow-sm`, `--shadow`, `--shadow-lg`; `--focus` ring
  (`0 0 0 3px color-mix(in srgb, var(--accent) 35%, transparent)`).
- Type: base 14 px, `h1` 20 px/600, `h2` 15 px/600 with -0.01em tracking,
  `.small` 12 px, `.num` tabular numerals. System font stack.
- Spacing scale 4/8/12/16/24/32.

Base elements: buttons (`.btn`, `.btn.primary`, `.btn.ghost`, `.btn.danger`,
`.btn.sm`, icon-only `.btn.icon`), inputs/selects/textarea with a focus ring,
`.switch` (checkbox styled as a toggle), `.card` (panel with a header row),
`.tag` / `.pill` (status chips with `.ok`, `.warn`, `.danger`, `.accent`
variants), `.kbd`, `.empty` (empty-state block with an icon and one line),
`.table` (header row in `--panel-2`, hover row, right-aligned numeric column
`.num`), `.bar` (track plus fill, 6 px, rounded).

Components in `ui/src/lib/ui/`:

- `Card.svelte` (title, optional subtitle, optional `actions` snippet).
- `Empty.svelte` (icon, title, hint, optional action snippet).
- `Switch.svelte` (checked, label, onchange).
- `Segmented.svelte` (options, selected id, onchange; used for ranges).
- `Icon.svelte` (name from `icons.ts`, size).
- `StatTile.svelte` (label, value, sub, optional color).
- `Toasts.svelte` in-page notification stack and `ui/src/lib/notify.ts`
  with `notify.ok(text)` / `notify.err(text)`.
- `Tooltip.svelte` a positioned hover tooltip for the charts.

## 2. App shell (`App.svelte`)

- Left sidebar, 220 px, collapses to an icon rail under 900 px: brand (app
  icon + "Nagsworth"), nav items with icons (Today, History, Apps, Web,
  Rules, Settings), a bottom block with the live status.
- Live status block: colored dot + state (Active in `exe`, Idle 3m, Locked,
  Paused), then the controls: Break now (primary), Pause 30 min / Resume,
  Reading mode (switch).
- Main area: page title row (h1 plus a page toolbar slot), content below,
  max width 1200 px, padding 24 px.
- Connection banner at the top when the API is unreachable (retrying every
  2 s); no error text in the sidebar.
- Keyboard: `1`-`6` switch pages when no input is focused.

## 3. Today (`routes/Dashboard.svelte`)

- Toolbar: day navigator (previous, label, next, a date input for jumping),
  total active time, a "Live" chip when viewing today.
- Stat tiles row: Active, Work, Game, Web, Idle (today's totals; tiles use
  the category colors).
- Rules card: each rule as a row with name, scope chip, state chip (Due /
  Nagging / Snoozed 3:21 / Break 7:10 / Over limit / Off), progress bar
  colored by closeness (accent, warn from 80 %, danger at 100 %), and row
  actions on hover: Snooze, Skip, Start break. Schedule rules show their
  windows instead of a bar.
- Categories card: a horizontal stacked bar (one segment per category,
  proportional) above the per-category list.
- Timeline card: full-width track with hour ticks and labels every 1-3 h
  depending on width, segments colored by category or kind, a custom hover
  tooltip (app, title, start-end, duration), a legend row for the colors,
  a "now" marker on today.
- Apps card: ranked table with display name, exe name muted, category
  swatch, game tag, bar, duration, share %; "Show all" beyond 12 rows.

## 4. History (new `routes/History.svelte`)

- Range segmented control: 7, 14, 30, 90 days.
- Stacked bar chart per day by category (from `/stats/daily`), pure SVG, no
  chart library; hover tooltip per bar; y axis in hours; weekend columns
  tinted; clicking a day opens it in Today.
- Below: totals per category for the range, average per day, longest day.
- Nav item "History" between Today and Apps.

## 5. Apps (`routes/Apps.svelte`)

- Toolbar: search box with an icon, category kind filter (All / Work / Game /
  Web / Other), "Only games" toggle, sort (last seen / name / category).
- Table: app (swatch, display name editable inline, exe name and path
  muted), category select, Game switch, "auto" / "you" source chip, last
  seen relative ("2 h ago", full date in `title`).
- Categories card: list with swatch, name, kind chip, app count; new
  category form with a color input.
- Changes confirm with `notify.ok("Saved")`.

## 6. Web (`routes/Web.svelte`)

- Range segmented control; summary tiles (total web time, sites, top site).
- Ranked table: a letter avatar colored from the host hash, host, bar,
  duration, share.
- Empty state explains pairing with a button that jumps to Settings.

## 7. Rules (`routes/Rules.svelte`)

- Two sub-tabs: "Rules" (cards) and "Config file" (editor).
- Rule cards from `GET /rules` `rules[]` (typed in `api.ts` as `RuleDef`):
  name, id muted, enabled, trigger summary ("Every 50m, break 10m" / "2h per
  day, repeat every 15m" / "Max 45m continuous" / "Blocked 23:00-06:00"),
  scope chip, escalation ladder (toast, overlay after 90 s, lock after 5 m),
  snooze line, live state from the snapshot. Per-rule enable toggle writes
  `enabled = true/false` into that rule's `[[rules]]` block of the TOML
  text and saves, so the file stays the source of truth. "Edit in file"
  switches tab and scrolls to the block.
- Config file tab: editor with a line-number gutter synced to the textarea
  scroll, monospace, Save / Revert, unsaved indicator, `Ctrl+S` saves,
  validation error in a red banner.
- Nag history: relative time, rule name (looked up), kind as a chip,
  level or reason.

## 8. Settings (`routes/Settings.svelte`)

- Cards: Browser extension (pairing code shown big with a copy button and
  an expiry progress bar; paired browsers with version, last seen relative,
  "online" chip when seen under 2 min ago), Startup (autostart switch),
  Your data (range, export buttons, backup with the path and a copy button),
  Local API (URL with copy button, short explanation), About (name and
  version from `/health`, repository link).

## 9. Nag windows (`Nag.svelte`, `Pair.svelte`)

- Toast: card with an accent left stripe, rule name, message, buttons
  "Start break" (primary), "Snooze (2 left)", "Skip" as ghost; several nags
  stack. Entrance animation (slide up and fade, 180 ms). The window stays
  380x210 logical px; if more height is needed, change `TOAST_H` in
  `src-tauri/src/windows.rs`.
- Overlay: dimmed backdrop with a deep indigo radial gradient (not pure
  black), big countdown (tabular, 96 px) inside an SVG progress ring for
  breaks, rule name, message, actions, "Nagging for 1m 20s" muted. Esc
  triggers skip / end early.
- Pair window: the same code display as Settings, expiry bar, "New code",
  instructions as two numbered steps.

## 10. Checks

`npm run ui:check` and `npm run ui:build` clean; both color schemes checked
in the Vite dev server against a running app (`?port=&token=`) at widths
640, 900 and 1200; a Tauri debug build run hidden with screenshots of the
dashboard, toast and overlay.
