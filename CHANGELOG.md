# Changelog

Timestamps: `YYYY-MM-DD_HHMMSS_<unix epoch>` (Europe/Budapest). Current work: [PLAN.md](PLAN.md).

## 2026-11-03_113806_1793702286: M0 scaffold plus tracking, nags and extension

- Workspace with six crates (`an-core`, `an-platform`, `an-storage`,
  `an-tracker`, `an-rules`, `an-api`) and the Tauri app `an-app`; root files,
  licenses, README, AGENTS.md, docs (architecture, decisions D1-D16, backlog),
  `config/default.toml` with the example rules.
- Win32 platform: foreground window and process (cached by pid + creation
  time), idle time, fullscreen detection, UWP child-window resolution,
  package family, elevation, WTS lock/unlock and suspend/resume on a
  message-only window, `LockWorkStation`, Run-key autostart, Steam registry
  state. `probe` example to see what it sees.
- Storage actor over SQLite with the planned schema, incrementally updated
  daily totals, stats queries, nag log, rule counters, settings, pruning.
- Tracker: sampler, pure segmenter (flap merging, backdated idle, sleep gaps
  as `suspended`, day split, periodic flush), categorizer with game
  detection (known exes, Steam, launcher folders), web segmenter.
- Rules: TOML rule set with validation; pure engine for interval,
  daily_total, continuous and schedule triggers with escalation, snooze,
  skip, start break, reading mode and persisted counters.
- API: axum on 127.0.0.1:47821 (fallback ports), bearer token, 6-digit
  pairing, CORS for extensions and Tauri, extension events (idempotent,
  incognito dropped, URLs reduced to the origin by default), stats, apps,
  categories, rules GET/PUT with hot reload, nags, SSE, control routes.
- App: runtime composition root without Tauri types, tray (dashboard, break,
  pause, reading mode, pairing, quit), close-to-tray, single instance, toast
  and per-monitor overlay windows, `--hidden`/`--autostart`/`--no-tray`,
  log file in the data directory.
- UI (Svelte 5): Today (categories, rules progress, timeline, apps), Apps
  (categories, game flag, display names, custom categories), Web, Rules
  (config editor, nag history), Settings (pairing, API); toast, overlay and
  pairing pages.
- Extension (MV3, Chrome + Firefox from one build): tab/focus events, a
  persistent queue with backoff, heartbeat, pairing options page, incognito
  opt-in.
- Fixes found while testing: a sleep gap followed by activity reset the rest
  counter (gap now counts as rest first); nags escalated while the user was
  locked away (escalation now only advances while counting); a toast-level
  nag whose break started never got its overlay; bodyless POSTs with a form
  content type (PowerShell) were rejected; `state` collided with Svelte's
  `$state` rune.
- Notes: tests run the app on a hidden desktop with `--no-tray`, because the
  first hidden run put a tray icon on the real taskbar; `GetForegroundWindow`
  sees nothing from the hidden desktop, so foreground detection was checked
  with the probe on the real desktop. A plain `cargo build` of the app loads
  the UI from the Vite dev server; build with the Tauri CLI to embed it.
- Checks: 53 Rust tests, clippy and fmt clean, `ui:check` and `ext:check`
  clean, both extension manifests built.

## 2026-10-31_222834_1793482114: Project design and plan

- Chose the stack and architecture with the owner: Windows first with a
  platform trait for Linux, Rust + Tauri 2 single tray app built from six
  workspace crates, rusqlite storage, axum local HTTP API with a bearer token,
  Svelte 5 UI, MV3 browser extension. Decisions D1-D8 in PLAN.md.
- Wrote the full design into PLAN.md: crate contracts, SQLite schema, runtime
  wiring, rules model with an example config, API endpoints, extension design,
  milestones M0-M6, risks.
- No code yet. Toolchain checked: cargo/rustc 1.90, node 25.2, WebView2 present;
  Tauri CLI will be a project devDependency.
- Repository initialized with this plan as the first commit.
