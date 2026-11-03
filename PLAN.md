# Activity tracker + nagger: plan

Last updated: 2026-11-03_113806_1793702286

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

## Current status (2026-11-03_113806_1793702286)

M0 is complete, and most of M1-M4 came with it because the pure-logic
pieces were cheap to finish properly. Everything below is built and tested,
and the app was run on the hidden desktop (dashboard, break overlay on both
monitors, API smoke test).

Acceptance for M0, checked:
- `cargo build --workspace`, `cargo test --workspace` (53 tests), clippy
  (`--all-targets`) and `cargo fmt --check` clean; `npm run ui:check` and
  `npm run ext:check` clean.
- The app starts with tray, dashboard and single instance; `/v1/health`
  answers, `/v1/state` without a token is 401, with it shows the live
  activity. Foreground detection checked on the real desktop with
  `cargo run -p an-platform --example probe` (exe, title, fullscreen, Steam
  running app id).
- `npm run ext:build` emits `extension/dist/chrome` and `extension/dist/firefox`.
- Not checked by hand: `npm run tauri dev` with the Vite dev server (the
  `tauri build --debug --no-bundle` binary was run instead), the tray menu
  itself (tests run with `--no-tray`), loading the extension into a real
  browser.

Milestone coverage:
- **M1 Tracking**: done. Idle (backdated), lock and sleep gaps, WTS session
  + power events on a message-only window, UWP via ApplicationFrameHost child
  windows, package family names, daily totals, timeline and per-app
  dashboard, retention pruning, day split at `day_start`.
- **M2 Gaming**: mostly done. Steam `RunningAppID` + `steamapps\common`,
  launcher path heuristics, built-in known-exe list, categorization UI (apps
  table, custom categories), category stats. Missing: `appmanifest_*.acf`
  game names, user-editable `known_games.toml`.
- **M3 Rules + nags**: done. Engine with interval / daily_total /
  continuous / schedule, escalation, snooze limits, skip, start break,
  reading mode, counters persisted across restarts; toast and per-monitor
  overlay windows; `lock` via `LockWorkStation`; TOML rule editor with
  validation and hot reload; nag history.
- **M4 Extension**: done except store packaging. Pairing flow, queue with
  backoff, heartbeat, `WebSegmenter`, web stats page, incognito opt-in.
- **M5 Linux**, **M6 Polish**: not started (`linux.rs` is a stub).

## Next steps

1. Try it for real: `npm run tauri dev`, check the tray menu, load the
   extension in Chrome and Firefox and pair it, live with the defaults for a
   few days and tune `min_segment_ms`, `idle_after_s` and the rules.
2. Exclusive-fullscreen handling for overlays (see risks): detect via
   `is_fullscreen` + `SHQueryUserNotificationState`, fall back to a sound and
   tray notice, re-raise when the game loses the foreground.
3. M2 leftovers: Steam `appmanifest_*.acf` names as display names; a
   `known_games.toml` in the data directory merged with the built-in list.
4. Dashboard: week/month views (daily stacked bars from `/stats/daily`),
   per-rule enable toggle, autostart toggle (`Platform::set_autostart`).
5. M6: NSIS installer check, CI (Windows build + tests, `ui:check`,
   `ext:check`), code signing, DB backup/export.

## Later milestones

- **M5 Linux**: `LinuxPlatform` (X11 via `x11rb` + XScreenSaver idle; Wayland via
  `ext-idle-notify-v1` + foreign-toplevel/portal), logind lock via D-Bus, AppImage/deb.
- **M6 Polish**: NSIS installer, code signing, CI, DB backup/export, high-DPI
  overlay checks, extension store packaging.

## Risks and open questions

- **Win32 process info**: elevated processes may refuse
  `PROCESS_QUERY_LIMITED_INFORMATION`; fall back to `GetProcessImageFileNameW`,
  then `exe_name = "<elevated>"`. UWP apps show as `ApplicationFrameHost.exe`;
  resolve via child windows of another PID + `GetApplicationUserModelId`.
  Cache `ProcessInfo` by (pid, create_time).
- **Idle and gamepads**: `GetLastInputInfo` ignores XInput; mitigated by
  `idle_policy = ignore` for games, later an optional `XInputGetState` poll.
- **Overlays vs exclusive fullscreen**: always-on-top windows cannot cover
  exclusive-fullscreen D3D. Detect via window rect == monitor rect without
  caption/thick frame and `SHQueryUserNotificationState`; degrade to sound +
  tray notice and re-raise when the game loses foreground. Never use
  `SetForegroundWindow` tricks.
- **Tauri 2**: keep Tauri's runtime separate from the tracking runtime and
  cross only via channels in `nag_bridge.rs`; Per-Monitor V2 DPI means
  physical pixels when comparing rects.
- **Found while building**: anti-cheat protected games refuse token queries
  and show as elevated (harmless); a hidden-desktop process can still add a
  tray icon to the user's taskbar (use `--no-tray` in tests).
- **Open**: product name; whether `lock` should ever be on by default; store
  packaging identity for the extension; whether a headless service build is
  wanted before M5.
