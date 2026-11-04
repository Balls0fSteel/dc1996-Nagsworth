# Changelog

Timestamps: `YYYY-MM-DD_HHMMSS_<unix epoch>` (Europe/Budapest). Current work: [PLAN.md](PLAN.md).

## 2026-11-04_104856_1793785736: Hands-on time, input-gated gaming limits

- Design in docs/HANDS_ON_TIME.md (D24): a gaming limit should protect
  hands, so it counts only the `tracking.input_window_s` (5 s) after each
  key press, mouse move or controller input; the time a game is merely in
  front is still recorded and shown next to it.
- `ns-platform`: `Platform::gamepad_idle_time` and `set_gamepad_polling`
  (`tracking.gamepad`); Windows polls `XInputGetState` on the four slots
  with the documented dead zones (left 7849, right 8689, trigger 30) and
  re-probes empty slots every 3 s; a change counts as input and
  `Sample.idle_ms` becomes the smaller of keyboard/mouse and pad idle, so
  controller players no longer show as idle. `Caps.gamepad`. Linux: none
  yet.
- `ns-core`: `tracking.input_window_s` (clamped to 600), `hands_on_games`,
  `gamepad`; `Activity.hands_on`.
- `ns-tracker`: the segmenter credits each sample within the window to the
  active segment (`input_ms`, never past the segment's length, split at
  day boundaries, kept through debounced switches); `Action::Extend/Close`
  carry it; `Activity.hands_on` set per sample.
- `ns-storage`: migration `002_hands_on.sql` (`input_ms` on `segments`,
  `daily_app_totals`, `daily_kind_totals`), `extend_segment(id, end,
  input_ms)` keeps the totals in the same transaction, `input_ms` on every
  stats struct, `category_kind_input_ms`.
- `ns-rules`: `IdlePolicy::Input`; `Rule::idle_policy(hands_on_games)`
  defaults Game rules to `input` while the toggle is on; the engine counts
  `input` rules only while `activity.hands_on` (reading mode now only turns
  `pause` into `ignore`); `EngineCommand::SetHandsOnGames`; the snapshot
  carries `hands_on_games` and each rule's effective `idle_policy`.
- `ns-api`: `/state` shows `activity.hands_on` and `engine.hands_on_games`;
  the CSV export ends in an `input_ms` column.
- Runtime: the engine starts with the config's toggle and follows config
  saves; `set_gamepad_polling(cfg.tracking.gamepad)` at start.
- UI: Settings > Tracking card (switch, seconds per input, controller
  note) editing `[tracking]` through `ui/src/lib/toml.ts` and `PUT /rules`;
  the Game tile shows hands-on "of <presence> in front"; a "Hands-on" apps
  column; "Playing" / "In front" in the status block; the Rules idle chip
  reads "hands-on".
- `config/default.toml`: the shipped `gaming-daily` rule no longer sets
  `idle_policy = "ignore"`, so it follows the toggle. `tools/seed_demo.py`
  seeds hands-on shares (games 45-75 %, work 80-98 %).
- Implementation split over four Opus 5.5 agents (platform, tracker +
  storage, rules + API, UI) against the contract in the design doc; one
  agent's note led to the default.toml change above.
- Checked: `cargo fmt`, clippy `-D warnings`, 92 tests, `ui:check`,
  `ui:build`, the Linux cross-check, and the debug app run hidden on a
  fresh data directory (migrated to version 2). Not checked: the API on
  the running binary (the owner's Nagsworth was running; single instance)
  and a real controller.

## 2026-11-04_102439_1793784279: Auto updater and setup, after PaTorrent

- New crate `ns-update` (D22): `Version` (semver order, the tag and setup
  file names), `Manifest` from GitHub's releases JSON (newest release and
  pre-release, drafts skipped), `Checker` (first check 30 s after the start,
  then every 12 h; manual checks; the banner dismissed per version as
  `update.seen`), `download_setup` (the release's setup and `SHA256SUMS`
  over HTTPS with ureq/rustls, written as `.part`, hash-checked, renamed),
  and on Windows `installed` (the setup's Settings > Apps entry names this
  folder) and `run_setup` (the file locked with `FILE_SHARE_READ`,
  re-hashed, then `/S /UPDATE /R /WAITPID=<pid> /ARGS --hidden`).
  `NAGSWORTH_UPDATE_URL` and `NAGSWORTH_UPDATE_FILES` redirect both to a
  local server for tests; `NAGSWORTH_NO_UPDATE_CHECK` at compile time
  removes the check. 13 unit tests, including a local HTTP server for the
  listing and the download (good, bad checksum, cancelled, a planted hard
  link) and the locked start with a copy of `cmd.exe`.
- `src-tauri/src/update.rs`: the `Updater` state (timer thread, fetch and
  download threads, `update.check` / `update.beta` / `update.seen` in the
  settings table), the `update-status` event and nine `update_*` commands;
  "Install and restart" starts the setup and exits through Tauri's normal
  shutdown. `tauri-plugin-opener` opens the release page from Rust (no new
  capability for the webview).
- Dashboard: Settings > Updates card (status line, Check now, Download
  update with progress and Cancel, Install and restart, What's new / Open
  the release page, the two switches, the last check's time) and a banner
  in the shell with Update / Later; `ui/src/lib/update.svelte.ts` shares the
  state. Tray: "Check for updates…", reading "Update to <version>…" once one
  is known, opens the dashboard on Settings.
- Installer: `src-tauri/windows/hooks.nsh` (`installerHooks`) adds
  `/WAITPID=<pid>`, a wait of up to a minute before Tauri's running-app
  check, so the exiting app is never killed mid-write; `installMode`
  currentUser and the homepage set explicitly. D23 records why Tauri's
  updater plugin (minisign keys in CI) was not used.
- Release workflow rewritten: a `version` job (tag = Cargo.toml =
  tauri.conf.json = the three package.json), Windows and Linux build jobs
  that upload artifacts, the Windows job running
  `tools/windows_install_test.ps1` on the runner (now also the `/UPDATE`
  path: the setup must outwait a sleeper, keep the folder's files and
  restart the app), and a `release` job that writes `SHA256SUMS` over all
  files and makes the draft release (pre-release for a suffixed version).
- Docs: README "Updates", docs/RELEASING.md, docs/MANUAL_CHECKS.md "Update
  check", docs/ARCHITECTURE.md "Updates", AGENTS.md crate map.
- Checked: fmt, clippy `-D warnings`, 73 tests, `ui:check`, `ui:build`;
  `npm run tauri -- build --bundles nsis` (4.4 MiB setup, Tauri's NSIS with
  the hook). The release build run hidden against a local fake listing
  (`v9.9.9`) logged `update check: 9.9.9 is available` 30 s after the start
  and the dashboard showed the banner with "See the release" (an unpacked
  copy) and "Later". `tools/windows_install_test.ps1` on the clean Windows
  11 VM: 24/24 when run in the console session through a scheduled task
  (`/IT`); over SSH the `/R` restart has no desktop shell and the script now
  skips that check there. Found on the way: Windows PowerShell's
  `Start-Process -Wait` also waits for the children, so the restarted app
  kept the test waiting; the setup is now waited for with `WaitForExit()`.
  Not run by hand: the Settings card's Download and Install buttons on an
  installed copy (the pieces behind them are unit-tested and the setup side
  is the VM run), and the release workflow itself (needs a tag).

## 2026-11-04_093504_1793781304: UI design pass

- Design system in `ui/src/app.css` (light/dark tokens, buttons, switch,
  cards, pills, tables, empty states, focus rings) and components in
  `ui/src/lib/ui/` (Card, Empty, Switch, Segmented, StatTile, Tooltip,
  Toasts, Icon with inline SVG icons in `lib/icons.ts`), an in-page notice
  store (`lib/notify.svelte.ts`) and a page-toolbar slot (`lib/page.svelte.ts`).
- Shell: left sidebar with icons (icon rail under 900 px), live status
  block with Break now / Pause / Reading mode, connection banner, keys 1-6.
- Today: stat tiles, rules with scope and state chips, hover actions and
  colored progress, stacked category bar, timeline with hour ticks, hover
  tooltip, legend and "now" marker, ranked apps table. New History page:
  stacked per-day chart (7/14/30/90 days, pure SVG), totals, averages,
  click-through to a day. Apps: search, kind filter, games-only, sort,
  inline rename, switches, source chips. Web: range tiles, letter avatars,
  pairing empty state. Rules: rule cards (trigger summary, scope, idle
  policy, escalation ladder, snooze, live state, enable toggle that edits
  the TOML block) and a config editor with line numbers, Ctrl+S and a
  validation banner; nag history with chips. Settings: cards for the
  extension (code with copy button and expiry bar, online chips), startup,
  data export/backup, local API, about.
- Nag windows: toast card with accent stripe, entrance animation and up to
  three stacked cards; the page reports its content height and the app
  sizes the window to it (new `resize_toast` command, `place_toast` in
  `windows.rs`), so the transparent window never covers more than the
  cards. Overlay with an indigo gradient, SVG progress ring around the
  countdown, Esc to skip. Pairing window with steps, copy button and expiry
  bar.
- Fixed in the bridge: a nag already active or on a break when the app
  starts (persisted counters) never got a window, because the engine
  raised it before the Tauri bridge subscribed to the bus; the bridge now
  seeds its list from the engine snapshot (`NagBridge::seed`, tested).
  A break started from the tray or dashboard had no length in its
  `NagView`, so the overlay could not draw the ring; it now takes
  `break_ms` from the snapshot.
- `tools/seed_demo.py` fills a scratch database with demo data for UI
  checks (documented in AGENTS.md, with a note that `run_hidden.ps1`'s
  screenshot skips the toast's tool window).
- Checked: `ui:check`, `ui:build`, fmt, clippy `-D warnings`, 60 tests;
  all pages in the Vite dev server against the running app with seeded
  data, light and dark, at 640, 800 and 1200 px; the Tauri debug build on
  the hidden desktop: dashboard, toast (380x145 after the resize) and the
  break overlay with the ring on both monitors; pairing page at 420x300.

## 2026-11-04_081147_1793779907: First push and CI on GitHub

- Pushed `main` to https://github.com/Balls0fSteel/dc1996-Nagsworth. The
  first CI run (41796859072) passed: Linux in 3m49s, Windows in 5m44s.
- GitHub warns that `actions/checkout@v4` and `actions/setup-node@v4` target
  the deprecated Node 20 (forced onto Node 24 for now).

## 2026-11-04_080403_1793779443: GitHub repository

- The repository is https://github.com/Balls0fSteel/dc1996-Nagsworth (public,
  empty when added). Added it as remote `origin` and pointed the workspace
  `repository` field at it. History checked for keys and tokens before the
  first push: none found. Not pushed yet.

## 2026-11-04_080211_1793779331: Renamed to Nagsworth

- Product name chosen: Nagsworth (shortlisted and checked against crates.io,
  npm and GitHub; no GitHub repo uses the name).
- Crates `an-*` became `ns-*` (folders `crates/ns-*`, app crate `ns-app`).
  Binary, data folder, log file, autostart entry, Windows Run value, NSIS
  product (`Nagsworth_<version>_x64-setup.exe`), Tauri identifier
  `org.nagsworth.app`, extension name, Firefox id `nagsworth@nagsworth.org`,
  store zips, npm packages, the `/v1/health` name and the docs all follow.
  `AN_DATA_DIR` / `AN_PYTHON` became `NAGSWORTH_DATA_DIR` / `NAGSWORTH_PYTHON`.
  User-facing text says "Nagsworth"; identifiers use `nagsworth`. The local
  git-ignored `.bat` scripts were updated too.
- Not migrated: data under `%LOCALAPPDATA%\activity_nagger\` and an
  `activity_nagger` autostart entry from earlier builds stay as they are
  (nothing released yet). Older changelog entries keep the old name.
- Checked: fmt, clippy `-D warnings`, 59 tests, Linux clippy for
  `ns-platform`, `ui:check`, `ext:check`, `ext:build` (manifests carry the new
  name and id); a Tauri debug build ran hidden, `/v1/health` answered
  `"name":"nagsworth"` and the dashboard shows the new name. The Linux `.deb`
  and the NSIS installer were not rebuilt.

## 2026-11-03_231552_1793744152: Local build and run scripts

- Root helper scripts for Windows, git-ignored at the owner's request:
  `1_build.bat` (release or `debug`, no installer), `2_run.bat` (builds if
  needed, starts it), `3_build_installer.bat` (NSIS setup),
  `4_run_installer.bat` (the newest setup, switches such as `/S` passed on),
  `5_dev.bat` (`tauri dev`), and `_deps.bat` (npm installs when missing).
- Checked: 1 and 3 build; 2 started the release build on the hidden desktop
  (API answered); 4's setup lookup run as a dry copy (not installed here).

## 2026-11-03_230747_1793743667: Linux and Windows checked on the test VMs

- Xubuntu 26.04 VM: installed the Tauri build libraries, Node, sway, foot,
  wmctrl; workspace clippy `-D warnings` and all tests pass there.
- New `tools/linux_checks.sh` (14/14) and `tools/linux_app_check.sh` (8/8),
  described in AGENTS.md; `probe` gained `--session`, `--autostart`, `--lock`.
- Fixed, found on the VM: logind Lock/Unlock signals were never received (the
  watcher matched `session/auto`, but signals come from the session's real
  path; now resolved with `GetSessionByPID`); the first Wayland samples saw no
  toplevels (two roundtrips before the event thread starts); newer stable
  clippy flagged `chunks_exact(4)` (now `as_chunks`); the `.deb` listed
  `libwebkit2gtk-4.1-0` twice.
- The built Linux app under Xvfb tracked an xterm, rendered the break
  overlay and created its tray icon; the `.deb` installs, runs and removes.
- Windows 11 VM: `tools/windows_install_test.ps1` (new) passed with 0
  failures on the release installer.
- The Xubuntu VM hung swapping on a 6-job build (4 GB RAM); recovered with a
  VMware soft reset; builds there now use 2 jobs.
- Still open: a real Linux desktop session (panel tray), tray clicks and an
  exclusive-fullscreen game on Windows (the Windows VM's session is locked),
  GitHub-hosted CI. FreeBSD and macOS have no backend (D1).

## 2026-11-03_223423_1793741663: Manual check list, tray action log

- Tray menu actions are logged (`tray menu: <id>`), so a hand check of the
  menu can be confirmed from the log.
- docs/MANUAL_CHECKS.md: steps and pass criteria for what automated runs
  here cannot cover (tray menu clicks, a nag during an exclusive-fullscreen
  game, the Linux backend on a Linux desktop, GitHub CI and releases, pairing
  the extension by hand).

## 2026-11-03_121237_1793704357: Extension verified in real browsers, CI steps run locally

- Added `tools/ext_e2e.ps1` (+ `tools/seed_ext.py`): runs the debug app, a
  local page server and a headless browser with a throwaway profile on the
  hidden desktop; the test copy of the extension pairs itself through
  `/pair`. Edge 156 and Firefox 159 both PASS: tab events flushed and
  stored, heartbeat listed by `/state`.
- `web-ext` added as an extension devDependency for the Firefox run.
- Ran the CI Windows job's steps locally in order; all pass.
- Notes: branded Chrome 156 ignores `--load-extension`, so the Chromium run
  uses Edge with `--disable-features=DisableLoadExtensionCommandLineSwitch`.
  Still unverified: the Linux backend on Linux, GitHub-hosted CI, clicking
  the tray menu, a real exclusive-fullscreen game, signing.

## 2026-11-03_120113_1793703673: M2-M6 completed, `tauri dev` verified

- Verified the M0 acceptance item left open: `npm run tauri dev` (run on the
  hidden desktop) starts Vite, builds and runs the app, logs `tray icon
  created` (new log line) and renders the dashboard from the dev server.
- M2: Steam game names from `appmanifest_<id>.acf` across the library
  folders in `libraryfolders.vdf`, set once as display name; user
  `known_games.toml` in the data directory extends the built-in game list.
- M5: Linux backend in `an-platform/src/linux/`: X11/XWayland via x11rb
  (EWMH active window, `_NET_WM_PID`, fullscreen state, MIT-SCREEN-SAVER
  idle), Wayland via `ext-idle-notify-v1` and
  `wlr-foreign-toplevel-management`, logind session events and lock over
  zbus, XDG autostart, Steam `registry.vdf`; `tauri.linux.conf.json` for
  `.deb`/AppImage. Exe names now compare by stem so categorization works for
  Linux names. Cross-checked with clippy for `x86_64-unknown-linux-gnu`; not
  run on Linux (WSL unavailable here: Hyper-V is off).
- M6: autostart toggle in Settings (Tauri commands over
  `Platform::set_autostart`); `GET /export` (CSV, or JSON with web visits) and
  `POST /backup` (`VACUUM INTO` under `backups/`) with Settings buttons; CI
  workflow (Windows checks + Linux build/tests) and tag-triggered release
  workflow with optional Windows signing from secrets; docs/RELEASING.md;
  release NSIS installer built (not installed).
- M4: `npm run ext:package` zips both extension builds for the stores with a
  small dependency-free zip writer.
- Exclusive fullscreen: overlays are deferred while
  `SHQueryUserNotificationState` reports a D3D fullscreen app or a
  presentation; the bridge beeps once and the tray tooltip names the nag.
  The bridge now rechecks once per second.
- Fixed: the Vite config used `__dirname` (warning under Vite 8).
- Mixed-DPI check: overlays covered both monitors at their physical sizes.
- Checks: 59 tests, clippy `-D warnings` and fmt clean on Windows; Linux
  clippy clean for `an-platform`; UI and extension checks clean.

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
