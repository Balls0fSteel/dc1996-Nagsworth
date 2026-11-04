# Changelog

Timestamps: `YYYY-MM-DD_HHMMSS_<unix epoch>` (Europe/Budapest). Current work: [PLAN.md](PLAN.md).

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
