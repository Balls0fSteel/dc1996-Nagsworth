# Nagsworth: plan

Last updated: 2026-11-05_001500_1793834100

Name: **Nagsworth** (chosen 2026-11-04, D9): a polite butler who keeps a note
of your hours and clears his throat when a break is due. Identifiers use
`nagsworth` (binary, data folder, npm packages); crates use the prefix `ns-`.
The repository folder on this PC is still `L:\dev\foss\activity_nagger`.
GitHub: https://github.com/Balls0fSteel/dc1996-Nagsworth (public; remote `origin`).

## Vision

Two apps in one, source-available (D25), for a single user's PC:

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
| D7 | Defaults on open points: window titles are stored even when the extension is paired (`tracking.store_titles` toggle); `lock` level is supported but commented out in shipped rules; one DB per Windows user under `%LOCALAPPDATA%\nagsworth\`; license MIT OR Apache-2.0 at first, replaced by PolyForm Noncommercial (D25). |
| D8 | Tauri CLI is a project devDependency (`@tauri-apps/cli` in root `package.json`, run via `npm run tauri`), not a global install. |
| D9 | Product name Nagsworth (2026-11-04): crates `ns-*`, binary and data folder `nagsworth`, Tauri identifier `org.nagsworth.app`, Firefox id `nagsworth@nagsworth.org`, env vars `NAGSWORTH_DATA_DIR` / `NAGSWORTH_PYTHON`. Checked free on crates.io, npm and GitHub; domains, trademarks and the extension stores not checked. |

Environment (2026-10-31): cargo/rustc 1.90, node 25.2, npm, WebView2 156
present. Sibling `foss/libretorrent` is its own nested git repo, so this
project has its own repo too (the enclosing `L:\dev` repo only tracks the
dev-folder notes).

## Repository layout

```
nagsworth/
  Cargo.toml                 # [workspace] members = ["crates/*", "src-tauri"], resolver 2, [workspace.dependencies]
  rust-toolchain.toml  rustfmt.toml  .editorconfig  .gitignore  LICENSE.md
  package.json               # devDeps: @tauri-apps/cli; scripts: tauri, ui:dev, ui:build, ext:build
  AGENTS.md  PLAN.md  CHANGELOG.md  README.md
  docs/ARCHITECTURE.md  docs/DECISIONS.md  docs/BACKLOG.md
  config/default.toml        # shipped defaults + example rules, include_str!'d as fallback
  crates/
    ns-core/      domain types, Event + EventBus, Clock, Config
    ns-platform/  Platform trait; windows.rs, mock.rs, linux.rs (stub)
    ns-tracker/   Sampler, Segmenter, Categorizer, gaming/, WebSegmenter
    ns-rules/     RuleSet (TOML), Engine, NagState machine, ControlCommand
    ns-storage/   Db, migrations/001_init.sql, storage actor, query modules
    ns-api/       axum router, auth, pairing, routes
  src-tauri/                 # crate ns-app: main.rs, app.rs, tray.rs, windows.rs, commands.rs, runtime.rs, nag_bridge.rs
  ui/                        # Svelte 5 + Vite + TS → ui/dist (tauri frontendDist)
  extension/                 # MV3 WebExtension, TS + Vite → dist/chrome, dist/firefox
```

Dependency direction: `ns-core` ← `ns-platform`, `ns-storage` ← `ns-tracker`,
`ns-rules` ← `ns-api` ← `ns-app`.

Shared `[workspace.dependencies]`: tokio, serde, serde_json, toml, thiserror,
anyhow, tracing, tracing-subscriber, time, humantime-serde, rusqlite (bundled),
rusqlite_migration, axum, tower, tower-http, rand, base64, windows
(Win32_Foundation, Win32_UI_WindowsAndMessaging, Win32_UI_Input_KeyboardAndMouse,
Win32_System_Threading, Win32_System_RemoteDesktop, Win32_System_Shutdown,
Win32_Graphics_Gdi), tauri 2 (`tray-icon`), tauri-plugin-single-instance,
tauri-plugin-autostart, tauri-plugin-opener, tauri-build.

## Crate contracts

- **ns-core** (no I/O beyond config load): `ProcessInfo{pid,exe_name,exe_path,is_elevated,package_id}`,
  `WindowInfo{hwnd,title,process,is_fullscreen,monitor}`, `Sample{ts,fg,idle,session}`,
  `SessionState{Active,Locked,Suspended}`, `Segment{app_id,title_id,start,end,kind}`,
  `SegmentKind{Active,Idle,Locked,Suspended,Paused}`, `CategoryKind{Work,Game,Web,Other}`,
  `Event{Tracker,Web,Nag,Session,Control}`, `EventBus` (tokio broadcast wrapper),
  `Clock` trait + `SystemClock`/`MockClock`, `config::{Config,load,data_dir}`,
  `Timestamp` (unix ms), `LocalDay` (computed with `day_start`, default 04:00).
- **ns-platform**: `trait Platform: Send + Sync { foreground_window, idle_time,
  process_info, session_events, autostart, set_autostart, lock_workstation, caps }`,
  `create_platform()`, `MockPlatform::scripted(Vec<Sample>)`. `WindowsPlatform`
  owns a message-only window thread for `WTSRegisterSessionNotification` and
  `WM_POWERBROADCAST`.
- **ns-tracker**: `Sampler::spawn(platform, interval, tx)` (1 s poll; raw samples
  never stored); `Segmenter::run(rx)` opens/closes segments on (app, title, kind)
  change, merges flaps shorter than `min_segment_ms` (2000), flushes the open
  segment every 10 s, inserts a `suspended` gap when the clock jumps > 5 s;
  `TrackerEvent{SegmentOpened,SegmentTick,SegmentClosed,IdleStarted,IdleEnded,ForegroundChanged}`;
  `Categorizer` (user override > known games > launcher heuristic > default);
  `gaming::{SteamDetector, LauncherPathDetector, KnownExeList}`; `WebSegmenter`
  accrues web time only while a browser is foreground and not idle.
- **ns-rules** (pure logic + `Clock`, unit-testable): `Rule{id,name,enabled,scope,trigger,idle_policy,escalation,snooze}`,
  `Scope{Any,Category,App}`, `Trigger{Interval{every,break_for}, DailyTotal{limit,renag_every},
  Continuous{limit,reset_after}, Schedule{windows}}`, `IdlePolicy{Pause,Ignore}`
  (reading mode is a global override to `Ignore`; `Game` scope defaults to `Ignore`
  because `GetLastInputInfo` does not see gamepads), `NagLevel{Toast,Overlay,Lock}`,
  `NagState{Idle,Pending,Active{level},Snoozed{until},InBreak{until},Satisfied}`,
  `NagEvent{Raise,Escalate,Dismiss}`, `ControlCommand{Snooze,SkipBreak,StartBreakNow,
  PauseTracking,ResumeTracking,SetReadingMode,Reload}`, `Engine::run()`,
  `watch<EngineSnapshot>`. Lock/idle time counts as rest (Workrave semantics).
- **ns-storage** (`001_init.sql`): `categories`, `apps` (exe_name, exe_path,
  package_id, category_id, category_source auto|user, is_game, first/last_seen),
  `titles` (dedup), `segments` (app_id, title_id, kind, start_ts, end_ts, local_day;
  indexes on (local_day, app_id) and start_ts), `web_visits` (host, url, title,
  tab_id, start/end, local_day), `daily_app_totals`, `daily_web_totals` (upserted
  on segment close so range queries never scan segments), `nag_events`,
  `rule_state` (counters survive restart), `settings` (runtime kv incl. token).
  Timestamps unix ms UTC. WAL, `synchronous=NORMAL`. Size: 1-3k segments/day,
  roughly 40-110 MB/year worst case; `retention_days` (365) prunes segments while
  totals stay. Privacy: `tracking.store_titles = full|none`, `web.store_url = host|full`.
- **ns-api** (`http://127.0.0.1:47821/v1`, bearer token except `/health` and `/pair`;
  CORS for `chrome-extension://`, `moz-extension://`, `tauri://localhost`):
  `GET /health`; `POST /pair {code}` → `{token}`; `POST /ext/events` (batched tab
  events, idempotent by client_id+ts+tab_id); `POST /ext/heartbeat`; `GET /state`;
  `GET /stats/{apps,categories,daily,timeline,web}`; `GET /apps`, `PATCH /apps/{id}`;
  `GET/POST /categories`; `GET/PUT /rules` (TOML, validated, hot reload);
  `GET /nags`; `GET /events` (SSE); `POST /control/{snooze,skip,break/start,pause,resume,reading-mode}`.
- **ns-app / runtime wiring**: platform thread → broadcast `SessionEvent`;
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
  ns-platform --target x86_64-unknown-linux-gnu -D warnings` clean.
- `npm run tauri dev` (run hidden): Vite dev server, app build, log line
  `tray icon created`, dashboard rendered from the dev server with live rule
  progress.
- Debug and release builds run hidden: `/v1/health` public, `/v1/state` 401
  without a token, pairing, idempotent extension events, break start opens
  overlays on both monitors (1936x1089 and 3856x2169 physical, so mixed-DPI
  placement works), backup writes a database copy, CSV export downloads.
- `npm run tauri build` produced the NSIS installer
  (`nagsworth_0.1.0_x64-setup.exe`, 3.6 MiB). Not installed.
- `npm run ext:build` / `ext:package`: both manifests and both store zips
  (zip integrity checked).
- Linux on the Xubuntu 26.04 VM (2026-11-03_230747_1793743667): workspace clippy
  `-D warnings` and all tests pass; `tools/linux_checks.sh` 14/14 (X11:
  active window, switching, idle, fullscreen; Wayland on sway: focused
  toplevel, switching, idle, fullscreen; logind Lock/Unlock including
  `Session.Lock`; autostart file); `tools/linux_app_check.sh` 8/8 (the built
  app tracks an xterm, timeline, break overlay rendered by WebKitGTK, tray
  created); the `.deb` installs, runs and removes. Two backend bugs found
  and fixed (logind signals, first Wayland sample).
- Windows 11 VM (clean machine): `tools/windows_install_test.ps1` 0 failures
  (silent install, Settings > Apps entry, the installed app's API, silent
  uninstall removes everything).
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

Not verified (needs another machine, an account or the user; steps in
docs/MANUAL_CHECKS.md):
- Linux in a real desktop session (panel tray, KDE, GNOME); the VM runs
  were headless (Xvfb, headless sway).
- Pairing typed by hand in the extension popup, and web time accruing while
  a browser is really in front (the hidden desktop has no foreground window;
  the web segmenter's foreground logic is unit-tested).
- The tray menu clicked by hand (tray creation itself is logged).
- An exclusive-fullscreen game during a nag (deferral logic is unit-tested).
- The release workflow on GitHub (CI passes there: first run 41796859072,
  Linux 3m49s, Windows 5m44s), signing with a real certificate, store
  submissions.

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
- **M5 Linux**: done, checked on the Xubuntu VM. X11 via x11rb + MIT-SCREEN-SAVER,
  Wayland via `ext-idle-notify-v1` + `wlr-foreign-toplevel-management`,
  logind Lock/Unlock/PrepareForSleep and `Session.Lock`, XDG autostart,
  Steam `registry.vdf`; `.deb` + AppImage bundle config.
- **M6 Polish**: done. Autostart toggle, NSIS installer, CI workflow
  (Windows + Linux), release workflow with optional signing
  (docs/RELEASING.md), CSV/JSON export, database backup, mixed-DPI overlay
  check, exclusive-fullscreen deferral (beep + tray tooltip).

## UI pass (2026-11-04_093504_1793781304)

Done, see docs/UI_DESIGN.md: design tokens and components (`ui/src/lib/ui`),
sidebar shell, Today with stat tiles and a hoverable timeline, a new History
page (stacked daily chart, pure SVG), Apps with search/filter/inline rename,
Web with site avatars, Rules as cards with per-rule toggles plus the config
editor with line numbers and Ctrl+S, Settings cards, redesigned toast
(sized to content), overlay (progress ring) and pairing window. Checked in
the browser (both color schemes, 640/800/1200 px) and in the built app on
the hidden desktop (dashboard, toast, break overlay on both monitors).
Not checked by hand: the toast's entrance animation and the hover actions
with a real mouse; see docs/MANUAL_CHECKS.md.

## Updates and setup (2026-11-04_102439_1793784279)

Done, after PaTorrent's updater (docs/DECISIONS.md D22, D23;
docs/RELEASING.md "The app updating itself"): `crates/ns-update` (version
order, GitHub releases listing, `Checker`, hash-checked setup download,
install detection, the locked and re-checked setup start),
`src-tauri/src/update.rs` (state, timer, downloads, the `update-status`
event, the `update.*` settings), the Settings > Updates card, the dashboard
banner, the tray item, `src-tauri/windows/hooks.nsh` (`/WAITPID`), the
release workflow with a version check, the runner install/update/uninstall
test and `SHA256SUMS`. Checked: the fake-listing run (banner, log) and the VM's
install/update/uninstall test (24/24 in the console session). Open: the
first published release is the first real end-to-end run of the GitHub
side (the workflow's runner test runs only on a tag), and "Download update"
/ "Install and restart" on an installed copy have not been clicked by hand
(docs/MANUAL_CHECKS.md "Update check").

## Hands-on time (2026-11-04_104856_1793785736)

Done, see docs/HANDS_ON_TIME.md (D24): gaming limits count only the
`input_window_s` (5 s) after each keyboard, mouse or XInput controller
input, while "in front" time stays recorded next to it. Pieces: XInput poll
in `ns-platform` (dead zones, empty slots re-probed every 3 s, folded into
`Sample.idle_ms`), `input_ms` on segments and daily totals (migration 002),
the segmenter's per-sample credit, `idle_policy = "input"` and the
`hands_on_games` default for Game rules in the engine, `input_ms` on every
stats row and the CSV export, `Activity.hands_on` in `/state`, the Settings
> Tracking card (edits `[tracking]` in the config text), the Game tile and
the "Hands-on" apps column on Today, "Playing" / "In front" in the status
block. Verified: 92 tests, clippy, fmt, ui:check, the Linux cross-check;
the built app run hidden migrated a fresh database to version 2
(`segments.input_ms` present). Not verified: the API fields on the running
binary (the owner's own Nagsworth was running and holds the single-instance
mutex, so the test copy exits after a second; the fields are covered by the
ns-api and ns-storage tests) and a real controller (docs/MANUAL_CHECKS.md
"Hands-on gaming time"). Open: Linux gamepads (evdev), raw HID pads on
Windows, a hands-on series on History, `input_window_s` needs a restart
(the segmenter reads it once), a UI control for `tracking.gamepad`.

## Extension 0.2 brief (2026-11-04_133502_1793795702)

docs/EXTENSION_V2.md is a build brief for the next extension version, to be
handed to its own implementation session (Opus 5.5, medium effort). The
0.1 extension (M4) stays as it is; the brief covers tracking edge cases
(unreadable private windows, start-up heartbeat, worker restarts), faster
port discovery, a real toolbar popup (status, today's sites, rule
progress, controls, optional badge), options-page additions, vitest unit
tests, store readiness (`web-ext lint`, AMO source zip, listing, permission
and privacy texts) and a popup check in `tools/ext_e2e.ps1`. Out of scope:
Safari, per-site rules, in-browser nags, store submission.

## Extension 0.2 built (2026-11-04_140801_1793797681)

All seven work items of docs/EXTENSION_V2.md are in (CHANGELOG.md). The
extension's version stays 0.1.0 until the next release bumps every
package together (the release workflow requires one version). Firefox's
minimum is now 140 (AMO's `data_collection_permissions`). Open:

- The checks the harness cannot do: docs/MANUAL_CHECKS.md "Extension, by
  hand" (real toolbar popup and badge, controls, private-window hint, the
  Firefox popup, a stopped Chrome worker).
- Store screenshots (extension/store/screenshots.md) once the name is
  checked; the listing texts are written.
- The popup reads state once when it opens; it does not refresh while open.

## Break exercises brief (2026-11-04_222508_1793831108)

docs/EXERCISES.md is a build brief for exercises during breaks: a
catalogue of 28 items (14 seated micro items, 14 standing ones, four
behind `band` / `stairs` / `floor` flags), each graded A/B/C by the
published evidence for sitting-related cardiometabolic risk, neck and
shoulder pain, eye strain and stress; a pure planner in `ns-rules` that
builds a routine from the break length, the time sat since the last real
break (the walk scales 1 to 5 min after Diaz 2023) and the day's total,
with a seeded, persisted rotation; `NagEvent::BreakStarted.routine`, a
`routine` column on `nag_events`, `GET /v1/exercises`, the overlay step
list with per-step countdowns, the toast cue, and a Settings > Exercises
card. Left out on purpose: soleus push-ups as a headline, palming, neck
circles, nerve gliding. Nothing is built; open questions (a 30 min
"move" rule, audio cues, completion tracking, the band tier) are at the
end of the brief. Decided 2026-11-04_223500_1793831700: a third default rule
`move-break` (every 30 min, 2 min, toast, walk/legs items) ships, with
`micro-break` moved to every 20 min and `rest-break` to every 60 min.

## Break exercises built (2026-11-05_001500_1793834100)

All work items of docs/EXERCISES.md are in (CHANGELOG.md): the three
default rules, `config/exercises.toml` (28 items), `ns_rules::exercises`
(catalogue with user overrides, planner, rotation), the engine planning at
raise and at break start, `NagEvent::Raise` / `BreakStarted` carrying the
routine, migration 003 (`nag_events.routine`), the rotation in `settings`,
`GET /v1/exercises`, `EngineCommand::SetExercises` on config save, the
overlay step view (per-step countdown, Space / Next, the list), the toast
cue, Settings > Break exercises (switches, equipment, the catalogue with
per-item toggles), the Rules chips and the routines in the nag history,
docs and the seed script. Verified: 115 tests, clippy, fmt, ui:check, the
Linux cross-check; the built app on the hidden desktop served 28 items,
started a rest break over the API with a six-step routine (walk first,
breathing last) logged to the nag history, and rendered the overlay on
both monitors. Not checked by hand: the toast cue, Space / Next in the
overlay, the Settings card's switches (docs/MANUAL_CHECKS.md "Break
exercises"). Open (docs/EXERCISES.md): audio cues, completion tracking on
History, the band tier default, habits like drinking water; the user
catalogue file is read at start only.

## Next steps

0. Run the checks in docs/MANUAL_CHECKS.md, first "Break exercises" and
   "Hands-on gaming time" with a controller.
1. Use it for a few days: load the extension, pair it, tune
   `min_segment_ms`, `idle_after_s` and the rules.
2. CI is green on GitHub. Move `actions/checkout` and `actions/setup-node`
   off v4 (GitHub warns that Node 20 actions are deprecated).
3. Try it in a real Linux desktop session (tray in a panel, KDE, GNOME).
4. Check the Nagsworth name against domains, trademarks and the extension
   stores, then get a code signing certificate and submit
   the extension to the stores (docs/RELEASING.md).
3a. Extension 0.2 is built: run its manual checks, then take the store
   screenshots (extension/store/screenshots.md) with step 4.
4a. Tag `v0.1.0` (or bump first): the release workflow's runner test and
   the draft release with `SHA256SUMS`; publish it, then check that an
   installed 0.1.0 offers the next release (docs/MANUAL_CHECKS.md).
5. Backlog items: KDE Wayland window tracking, a structured rule editor
   (docs/BACKLOG.md).
6. Break exercises follow-ups decided 2026-11-04_231240_1793833960 (docs/EXERCISES.md "Open
   questions"): completion tracking on History, an opt-in step-change tick
   (off by default), two or three `habits` items (drink water, look out of
   a window), off by default. Band tier stays off until ticked. The user
   catalogue file is still read at start only (known limitation).

## Risks and open questions

- **Win32 process info**: elevated processes may refuse
  `PROCESS_QUERY_LIMITED_INFORMATION`; fall back to `GetProcessImageFileNameW`,
  then `exe_name = "<elevated>"`. UWP apps show as `ApplicationFrameHost.exe`;
  resolve via child windows of another PID + `GetApplicationUserModelId`.
  Cache `ProcessInfo` by (pid, create_time).
- **Idle and gamepads**: `GetLastInputInfo` ignores XInput, so the sampler
  also polls `XInputGetState` (D24); pads that are not XInput (DualShock
  over plain HID, Linux) still look idle, and a Game rule set to
  `idle_policy = "ignore"` keeps the old behaviour.
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
- **Decided 2026-11-04_231240_1793833960**: `lock` is never on in a default rule (opt-in
  only); the extension's store identity (gecko id, listing name) is part of
  the name check in next step 4.
- **Open**: whether a headless service build (tracking + API, no WebView)
  is wanted at all; M5 is done, so the old "before M5" no longer applies.
