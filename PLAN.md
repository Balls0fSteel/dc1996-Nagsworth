# Activity tracker + nagger: plan

Last updated: 2026-11-03_121237_1793704357

Working name: `activity_nagger` (crate prefix `an-`). A real name is still to be
chosen; the working name is used everywhere so a later rename is a
search-and-replace.

## Vision

Two apps in one, open source, for a single user's PC:

1. **Activity tracker**: which apps are in the foreground and for how long
   (process, window title, idle detection), how much time goes to gaming
   (Steam/launcher/known-exe detection plus user categorization), and which
   websites are visited (a browser extension reports active-tab changes).
2. **Nagger**: a modern, app-aware Workrave. Interval breaks, micro-breaks,
   Workrave-style reading mode (timers run on wall clock, not input), daily
   limits per category ("2h of gaming"), schedules ("no gaming 23:00-06:00"),
   with escalating nag UI: toast → fullscreen overlay → optional workstation
   lock. Workrave is unmaintained, dated, and tracks nothing.

## Decisions (owner-approved 2026-10-31)

| ID | Decision |
|---|---|
| D1 | Windows first, Linux later. A `Platform` trait isolates OS code so an X11/Wayland backend can be added. |
| D2 | Rust + Tauri 2. One tray process built from workspace crates; a headless service split stays possible because the composition root (`runtime.rs`) uses no Tauri types. |
| D3 | SQLite via `rusqlite` (`bundled`) + `rusqlite_migration`, one storage actor thread owning the connection. Single-writer workload; no need for sqlx's pool or offline query metadata; bundled SQLite pins the version on Linux. |
| D4 | Frontend: Svelte 5 + TypeScript + Vite, multi-page (`index.html` dashboard, `toast.html`, `overlay.html`, `pair.html`). |
| D5 | Browser extension (Manifest V3, Chrome + Firefox) talks to a local HTTP API (`axum`) on `127.0.0.1:47821` with a bearer token; the same API serves the dashboard and future clients. Fixed port with a short fallback list the extension probes (extensions cannot read a discovery file). |
| D6 | No global hooks, no injection, no reading other processes' memory (antivirus-friendly). Idle via `GetLastInputInfo`; Steam detection via `HKCU\Software\Valve\Steam\RunningAppID`. The `lock` escalation level calls `LockWorkStation()` rather than trying to build an inescapable overlay. |
| D7 | Defaults on open points: window titles are stored even when the extension is paired (`tracking.store_titles` toggle); `lock` level is supported but commented out in shipped rules; one DB per Windows user under `%LOCALAPPDATA%\activity_nagger\`; license MIT OR Apache-2.0. |
| D8 | Tauri CLI is a project devDependency (`@tauri-apps/cli` in root `package.json`, run via `npm run tauri`), not a global install. |

Environment (2026-10-31): cargo/rustc 1.90, node 25.2, npm, WebView2 156
present. Sibling `foss/libretorrent` is its own nested git repo, so this
project has its own repo too (the enclosing `L:\dev` repo only tracks the
dev-folder notes).

## Repository layout

```
activity_nagger/
  Cargo.toml                 # [workspace] members = ["crates/*", "src-tauri"], resolver 2, [workspace.dependencies]
  rust-toolchain.toml  rustfmt.toml  .editorconfig  .gitignore  LICENSE-MIT  LICENSE-APACHE
  package.json               # devDeps: @tauri-apps/cli; scripts: tauri, ui:dev, ui:build, ext:build
  AGENTS.md  PLAN.md  CHANGELOG.md  README.md
  docs/ARCHITECTURE.md  docs/DECISIONS.md  docs/BACKLOG.md
  config/default.toml        # shipped defaults + example rules, include_str!'d as fallback
  crates/
    an-core/      domain types, Event + EventBus, Clock, Config
    an-platform/  Platform trait; windows.rs, mock.rs, linux.rs (stub)
    an-tracker/   Sampler, Segmenter, Categorizer, gaming/, WebSegmenter
    an-rules/     RuleSet (TOML), Engine, NagState machine, ControlCommand
    an-storage/   Db, migrations/001_init.sql, storage actor, query modules
    an-api/       axum router, auth, pairing, routes
  src-tauri/                 # crate an-app: main.rs, app.rs, tray.rs, windows.rs, commands.rs, runtime.rs, nag_bridge.rs
  ui/                        # Svelte 5 + Vite + TS → ui/dist (tauri frontendDist)
  extension/                 # MV3 WebExtension, TS + Vite → dist/chrome, dist/firefox
```

Dependency direction: `an-core` ← `an-platform`, `an-storage` ← `an-tracker`,
`an-rules` ← `an-api` ← `an-app`.

Shared `[workspace.dependencies]`: tokio, serde, serde_json, toml, thiserror,
anyhow, tracing, tracing-subscriber, time, humantime-serde, rusqlite (bundled),
rusqlite_migration, axum, tower, tower-http, rand, base64, windows
(Win32_Foundation, Win32_UI_WindowsAndMessaging, Win32_UI_Input_KeyboardAndMouse,
Win32_System_Threading, Win32_System_RemoteDesktop, Win32_System_Shutdown,
Win32_Graphics_Gdi), tauri 2 (`tray-icon`), tauri-plugin-single-instance,
tauri-plugin-autostart, tauri-plugin-opener, tauri-build.

## Crate contracts

- **an-core** (no I/O beyond config load): `ProcessInfo{pid,exe_name,exe_path,is_elevated,package_id}`,
  `WindowInfo{hwnd,title,process,is_fullscreen,monitor}`, `Sample{ts,fg,idle,session}`,
  `SessionState{Active,Locked,Suspended}`, `Segment{app_id,title_id,start,end,kind}`,
  `SegmentKind{Active,Idle,Locked,Suspended,Paused}`, `CategoryKind{Work,Game,Web,Other}`,
  `Event{Tracker,Web,Nag,Session,Control}`, `EventBus` (tokio broadcast wrapper),
  `Clock` trait + `SystemClock`/`MockClock`, `config::{Config,load,data_dir}`,
  `Timestamp` (unix ms), `LocalDay` (computed with `day_start`, default 04:00).
- **an-platform**: `trait Platform: Send + Sync { foreground_window, idle_time,
  process_info, session_events, autostart, set_autostart, lock_workstation, caps }`,
  `create_platform()`, `MockPlatform::scripted(Vec<Sample>)`. `WindowsPlatform`
  owns a message-only window thread for `WTSRegisterSessionNotification` and
  `WM_POWERBROADCAST`.
- **an-tracker**: `Sampler::spawn(platform, interval, tx)` (1 s poll; raw samples
  never stored); `Segmenter::run(rx)` opens/closes segments on (app, title, kind)
  change, merges flaps shorter than `min_segment_ms` (2000), flushes the open
  segment every 10 s, inserts a `suspended` gap when the clock jumps > 5 s;
  `TrackerEvent{SegmentOpened,SegmentTick,SegmentClosed,IdleStarted,IdleEnded,ForegroundChanged}`;
  `Categorizer` (user override > known games > launcher heuristic > default);
  `gaming::{SteamDetector, LauncherPathDetector, KnownExeList}`; `WebSegmenter`
  accrues web time only while a browser is foreground and not idle.
- **an-rules** (pure logic + `Clock`, unit-testable): `Rule{id,name,enabled,scope,trigger,idle_policy,escalation,snooze}`,
  `Scope{Any,Category,App}`, `Trigger{Interval{every,break_for}, DailyTotal{limit,renag_every},
  Continuous{limit,reset_after}, Schedule{windows}}`, `IdlePolicy{Pause,Ignore}`
  (reading mode is a global override to `Ignore`; `Game` scope defaults to `Ignore`
  because `GetLastInputInfo` does not see gamepads), `NagLevel{Toast,Overlay,Lock}`,
  `NagState{Idle,Pending,Active{level},Snoozed{until},InBreak{until},Satisfied}`,
  `NagEvent{Raise,Escalate,Dismiss}`, `ControlCommand{Snooze,SkipBreak,StartBreakNow,
  PauseTracking,ResumeTracking,SetReadingMode,Reload}`, `Engine::run()`,
  `watch<EngineSnapshot>`. Lock/idle time counts as rest (Workrave semantics).
- **an-storage** (`001_init.sql`): `categories`, `apps` (exe_name, exe_path,
  package_id, category_id, category_source auto|user, is_game, first/last_seen),
  `titles` (dedup), `segments` (app_id, title_id, kind, start_ts, end_ts, local_day;
  indexes on (local_day, app_id) and start_ts), `web_visits` (host, url, title,
  tab_id, start/end, local_day), `daily_app_totals`, `daily_web_totals` (upserted
  on segment close so range queries never scan segments), `nag_events`,
  `rule_state` (counters survive restart), `settings` (runtime kv incl. token).
  Timestamps unix ms UTC. WAL, `synchronous=NORMAL`. Size: 1-3k segments/day,
  roughly 40-110 MB/year worst case; `retention_days` (365) prunes segments while
  totals stay. Privacy: `tracking.store_titles = full|none`, `web.store_url = host|full`.
- **an-api** (`http://127.0.0.1:47821/v1`, bearer token except `/health` and `/pair`;
  CORS for `chrome-extension://`, `moz-extension://`, `tauri://localhost`):
  `GET /health`; `POST /pair {code}` → `{token}`; `POST /ext/events` (batched tab
  events, idempotent by client_id+ts+tab_id); `POST /ext/heartbeat`; `GET /state`;
  `GET /stats/{apps,categories,daily,timeline,web}`; `GET /apps`, `PATCH /apps/{id}`;
  `GET/POST /categories`; `GET/PUT /rules` (TOML, validated, hot reload);
  `GET /nags`; `GET /events` (SSE); `POST /control/{snooze,skip,break/start,pause,resume,reading-mode}`.
- **an-app / runtime wiring**: platform thread → broadcast `SessionEvent`;
  Sampler → mpsc `Sample` → Segmenter → `StorageHandle` (mpsc + oneshot) and
  `EventBus`; Engine subscribes to the bus + 1 s tick + mpsc `ControlCommand`,
  publishes `NagEvent` → `nag_bridge.rs` → toast/overlay windows (one overlay
  per monitor, `always_on_top`, undecorated, transparent, skip taskbar);
  `watch<EngineSnapshot>` → API and UI. Tray: Open dashboard, Pause 30 min,
  Reading mode, Quit. Hidden main window, close-to-tray, single instance.
- **extension**: `background.ts` listens to `tabs.onActivated`, `tabs.onUpdated`
  (active tab, url/title change), `windows.onFocusChanged`, `tabs.onRemoved`,
  `idle.onStateChanged`; `queue.ts` is a storage.local FIFO (cap 5000) flushed
  every 15 s or at 20 items with backoff; heartbeat every 60 s carries the active
  tab; incognito dropped unless opted in; pairing by a 6-digit code shown by the
  app. `manifest.base.json` + `scripts/build-manifests.mjs` emit Chrome
  (service_worker) and Firefox (background.scripts + gecko id) manifests.

## Example config (`config/default.toml`)

```toml
[general]
day_start = "04:00"
reading_mode = false

[tracking]
poll_ms = 1000
idle_after_s = 120
min_segment_ms = 2000
store_titles = "full"

[api]
port = 47821
bind = "127.0.0.1"

[[rules]]
id = "rest-break"
name = "Take a 10 minute break"
trigger = { kind = "interval", every = "50m", break_for = "10m" }
scope = "any"
idle_policy = "pause"
escalation = [ { after = "0s", level = "toast" }, { after = "90s", level = "overlay" } ]  # , { after = "5m", level = "lock" }
snooze = { duration = "5m", max_per_day = 3 }

[[rules]]
id = "micro-break"
trigger = { kind = "interval", every = "10m", break_for = "30s" }
escalation = [ { after = "0s", level = "toast" } ]

[[rules]]
id = "gaming-daily"
name = "Gaming limit"
scope = { category = "Game" }
trigger = { kind = "daily_total", limit = "2h", renag_every = "15m" }
idle_policy = "ignore"
escalation = [ { after = "0s", level = "toast" }, { after = "10m", level = "overlay" } ]

[[rules]]
id = "no-late-gaming"
scope = { category = "Game" }
trigger = { kind = "schedule", windows = ["23:00-06:00"] }
escalation = [ { after = "0s", level = "overlay" } ]
```

## Current status (2026-11-03_120113_1793703673)

All milestones M0-M6 are implemented. What could be run on this Windows PC
was run; what needs another machine or an account is listed under "Not
verified".

Verified here:
- `cargo test --workspace` (59 tests), `cargo clippy --workspace
  --all-targets -D warnings`, `cargo fmt --check`, `npm run ui:check`,
  `npm run ext:check`: clean. Linux backend: `cargo +1.90.0 clippy -p
  an-platform --target x86_64-unknown-linux-gnu -D warnings` clean.
- `npm run tauri dev` (run hidden): Vite dev server, app build, log line
  `tray icon created`, dashboard rendered from the dev server with live rule
  progress.
- Debug and release builds run hidden: `/v1/health` public, `/v1/state` 401
  without a token, pairing, idempotent extension events, break start opens
  overlays on both monitors (1936x1089 and 3856x2169 physical, so mixed-DPI
  placement works), backup writes a database copy, CSV export downloads.
- `npm run tauri build` produced the NSIS installer
  (`activity_nagger_0.1.0_x64-setup.exe`, 3.6 MiB). Not installed.
- `npm run ext:build` / `ext:package`: both manifests and both store zips
  (zip integrity checked).
- Extension in real browsers (`tools/ext_e2e.ps1`, 2026-11-03_121237_1793704357): headless Edge
  156 (`--load-extension`, tabs driven over DevTools) and Firefox 159 (via
  `web-ext run`, temporary add-on), each with a throwaway profile. Both paired
  with a fresh code, flushed queued tab events (8 and 2 stored) and sent a
  heartbeat the app lists (`edge 0.1.0`, `firefox 0.1.0`). PASS for both.
- CI Windows job: every step of `.github/workflows/ci.yml` run locally in
  order (`npm ci` x3, UI build/check, extension check/build, fmt, clippy
  `-D warnings`, tests): all exit 0.
- Real desktop (read-only probe): foreground exe/title/fullscreen, Steam
  running app id, `SHQueryUserNotificationState`; the Steam manifest lookup
  finds the running game's name in the main library.

Not verified (needs another machine, an account or the user):
- The Linux backend on a real Linux desktop (no Linux machine; WSL is not
  available here). Parsers are unit-tested; X11/Wayland/logind code only
  compiles.
- Pairing typed by hand in the extension popup, and web time accruing while
  a browser is really in front (the hidden desktop has no foreground window;
  the web segmenter's foreground logic is unit-tested).
- The tray menu clicked by hand (tray creation itself is logged).
- An exclusive-fullscreen game during a nag (deferral logic is unit-tested).
- CI and release workflows on GitHub (no remote yet; the Windows job's steps
  pass locally, the Linux job cannot run here), signing with a real
  certificate, store submissions.

Milestone coverage:
- **M0 Scaffold**: done, acceptance items above.
- **M1 Tracking**: done. Idle (backdated), lock and sleep gaps, WTS session
  and power events, UWP resolution, package family names, daily totals,
  timeline and per-app dashboard, retention pruning, day split.
- **M2 Gaming**: done. Steam `RunningAppID` + `steamapps\common`, Steam
  names from `appmanifest_<id>.acf` across library folders, launcher path
  heuristics, built-in list plus user `known_games.toml`, categorization UI,
  category stats.
- **M3 Rules + nags**: done. Four trigger kinds, escalation, snooze limits,
  skip, start break, reading mode, persisted counters; toast and
  per-monitor overlays; `lock` via `LockWorkStation`; TOML rule editor with
  validation and hot reload; nag history.
- **M4 Extension**: done. Pairing, persistent queue with backoff,
  heartbeat, `WebSegmenter`, web stats page, incognito opt-in; Chrome and
  Firefox store zips (`npm run ext:package`).
- **M5 Linux**: done (unverified on Linux). X11 via x11rb + MIT-SCREEN-SAVER,
  Wayland via `ext-idle-notify-v1` + `wlr-foreign-toplevel-management`,
  logind Lock/Unlock/PrepareForSleep and `Session.Lock`, XDG autostart,
  Steam `registry.vdf`; `.deb` + AppImage bundle config.
- **M6 Polish**: done. Autostart toggle, NSIS installer, CI workflow
  (Windows + Linux), release workflow with optional signing
  (docs/RELEASING.md), CSV/JSON export, database backup, mixed-DPI overlay
  check, exclusive-fullscreen deferral (beep + tray tooltip).

## Next steps

1. Use it for a few days: load the extension, pair it, tune
   `min_segment_ms`, `idle_after_s` and the rules.
2. Push to GitHub and let CI run; fix anything the Linux job finds.
3. Run it on a Linux desktop (X11 and a wlroots compositor) and fix what the
   untested backend gets wrong.
4. Choose the product name, then get a code signing certificate and submit
   the extension to the stores (docs/RELEASING.md).
5. Backlog items: week/month charts, per-rule toggle, KDE Wayland window
   tracking (docs/BACKLOG.md).

## Risks and open questions

- **Win32 process info**: elevated processes may refuse
  `PROCESS_QUERY_LIMITED_INFORMATION`; fall back to `GetProcessImageFileNameW`,
  then `exe_name = "<elevated>"`. UWP apps show as `ApplicationFrameHost.exe`;
  resolve via child windows of another PID + `GetApplicationUserModelId`.
  Cache `ProcessInfo` by (pid, create_time).
- **Idle and gamepads**: `GetLastInputInfo` ignores XInput; mitigated by
  `idle_policy = ignore` for games, later an optional `XInputGetState` poll.
- **Overlays vs exclusive fullscreen**: always-on-top windows cannot cover
  exclusive-fullscreen D3D. Implemented: `SHQueryUserNotificationState`
  defers overlays with a beep and a tray tooltip until the game leaves
  exclusive mode. Never use `SetForegroundWindow` tricks.
- **Tauri 2**: keep Tauri's runtime separate from the tracking runtime and
  cross only via channels in `nag_bridge.rs`; Per-Monitor V2 DPI means
  physical pixels when comparing rects.
- **Found while building**: anti-cheat protected games refuse token queries
  and show as elevated (harmless); a hidden-desktop process can still add a
  tray icon to the user's taskbar (use `--no-tray` in tests).
- **Open**: product name; whether `lock` should ever be on by default; store
  packaging identity for the extension; whether a headless service build is
  wanted before M5.
