# Changelog

Timestamps: `YYYY-MM-DD_HHMMSS_<unix epoch>` (Europe/Budapest). Current work: [PLAN.md](PLAN.md).

## 2026-11-05_092738_1793870858: Headless phase 2: nag board and API, the GUI as a client

- `ns_core::view`: `NagView`, `NagsView`, `UpdateStatus` (+ views),
  `AppEvent`; `Event` gains `Nags`, `Update`, `App` and `Event::source()`.
- `ns_runtime::nag_board`: the bridge's logic (one countdown for several
  breaks, toast vs overlay, seeding from the snapshot, exclusive-fullscreen
  deferral with the beep) as a pure `NagBoard` plus a task that publishes
  `NagsView` on a watch and the bus. It subscribes before the engine task
  starts. The bridge's three tests moved with it.
- `ns_runtime::update`: the updater, unchanged in behaviour, without Tauri:
  state changes go out as `Event::Update`, starting the setup publishes
  `AppEvent::ExitForUpdate` (the app exits on it).
- `ns-api`: `Host` trait (autostart, updates; the runtime implements it
  with the platform and the updater), `GET /nags/current`,
  `GET/PUT /autostart`, `GET /update`, `POST /update/{check,download,cancel,
  install,dismiss}`, `PUT /update/settings`, `/events?sources=a,b`. A host
  error is 409 with `{error}`.
- Tauri shell: `nag_bridge.rs` only opens and closes windows from the
  board's watch and exits on `ExitForUpdate`; `update.rs` is gone; commands
  left: `get_api_info`, `open_dashboard`, `resize_toast`,
  `open_release_page` (refuses URLs outside this project's releases).
  `ns-storage` is no longer a direct dependency.
- UI: `subscribeEvents` (fetch-streamed SSE with the bearer token,
  reconnects and reloads), the nag page on `/nags/current` + `nags` events
  + `/control/*`, autostart and updates on HTTP (so they also work in a
  plain browser), the step tick's setting from `/exercises`. The overlay no
  longer reads its Tauri window label outside the app.
- Verified: 109 tests (new: the endpoints with a fake host), clippy
  `-D warnings`, fmt, `ui:check`; on the hidden desktop the built app
  reported the overlay wanted during a break and cleared after skip, the
  filtered stream carried only `nags` and `update` events in order, and
  `/update` and `/autostart` answered (autostart was only read, never set:
  that writes the real Run key). In the browser against the running app the
  overlay page showed a break started after it loaded, Esc ended it
  (`ended_early`) and the page emptied, and Settings' "Check now" showed
  the answer from the stream. Not checked: the nag windows inside the app
  itself (the capture tool pictures only the dashboard), the tray tooltip
  during a deferred overlay, an update install.

## 2026-11-05_091535_1793870135: Headless phase 1: `ns-runtime` crate

- `src-tauri/src/runtime.rs` moved unchanged to `crates/ns-runtime/src/lib.rs`
  (composition root, side effects, retention, and its start/serve/shutdown
  test with `MockPlatform`); `ns-app` depends on it and no longer on
  `ns-api`, `ns-tracker`, `anyhow` or `serde_json` directly. `cargo tree -p
  ns-runtime` has no Tauri, WebView or wry crate. No behaviour change.
- Verified: 108 tests, clippy `-D warnings`, fmt; the built app started on
  the hidden desktop, served `/v1/state` (5 rules) and `/v1/stats/breaks`.
  The Linux cross-check is not possible for `ns-runtime` on this machine
  (bundled SQLite needs a Linux C cross-compiler); it adds no platform code.
- Docs: AGENTS.md crate map and dependency direction, ARCHITECTURE.md,
  PLAN.md layout, docs/HEADLESS.md phase 1 status.

## 2026-11-05_091205_1793869925: Headless-first decided

- The owner took all five proposals in docs/HEADLESS.md: the service owns
  the tray, toasts go to OS notifications and overlays start the GUI when
  it is closed, one installer, `nagsworthd` + `nagsworth`, and v0.1.0 is
  tagged first with phases 1-2 next and the split in 0.2.0. Recorded as
  D26 in docs/DECISIONS.md; PLAN.md step 7 updated.

## 2026-11-04_232332_1793834612: Break completion tracking, step tick, habits; headless brief

- Fix: Esc ("end the break early") during a running break did nothing: the
  engine only handled `SkipBreak` for raised or snoozed nags. It now ends
  the break with the new `DismissReason::EndedEarly` (counters reset as
  for a finished break, since the user did rest).
- Completion tracking: `Db::breaks` pairs each `break_started` row with
  the next dismiss of the same rule by row id (the start's `at` is the wall
  clock at logging, the dismiss's the engine tick, so ordering by `at`
  could mis-pair); `GET /v1/stats/breaks?from&to` counts started,
  completed, ended early and breaks with a routine per local day (a day of
  margin on each side for offset changes). History has a "Breaks" card;
  it hides itself against an older service without the endpoint.
- Step tick: `[exercises] step_sound` (default off), a Settings switch,
  the `nag_prefs` command; a short WebAudio sine tick when the routine
  moves past its first step, played by the `overlay-0` window only so
  several monitors do not echo. `Runtime.config` is now public for it.
- Habits: `drink-water` and `daylight` (grade C, tag `habits`) in
  `config/exercises.toml` (30 items), `[exercises] habits` (default off), a
  Settings switch. With it on they skip a rule's tag filter in breaks of a
  minute or more, at most one per routine, and every long break gets one;
  micro-breaks keep to their tags.
- `tools/seed_demo.py` seeds 14 paired breaks (a quarter ended early).
- Verified: 108 tests, clippy `-D warnings`, fmt, `ui:check`; the built app
  on the hidden desktop started a rest break over the API with habits on
  (routine walk, drink-water, ear to shoulder, near-far, wall slides,
  cyclic sighing), `/control/skip` ended it (state idle) and
  `/stats/breaks` reported one ended early; against a seeded database the
  History card and the two Settings switches render, and switching habits
  off saved to the live config. Not checked: the tick by ear, Esc in a
  real overlay (docs/MANUAL_CHECKS.md "Break exercises" 6-7).
- docs/HEADLESS.md: headless-first proposal (`ns-runtime` crate,
  `nagsworthd` per-user service, the GUI as an API client, discovery file,
  nag board in the service, five phases, five open questions).

## 2026-11-04_231240_1793833960: Open questions reviewed

- `lock` stays opt-in; no default rule will use it.
- Break exercises: completion tracking on History is to be built now; an
  opt-in step-change tick (off by default); a few `habits` items (drink
  water, look out of a window), off by default; the band tier stays off
  until ticked. Recorded in docs/EXERCISES.md and PLAN.md.
- Extension store identity folded into the name check (PLAN.md step 4).
- User catalogue reload on save went to docs/BACKLOG.md.
- Still open: whether a headless service build is wanted at all.

## 2026-11-05_001500_1793834100: Break exercises (docs/EXERCISES.md)

- **Default rules** (`config/default.toml`): `micro-break` every 20 min for
  30 s with `exercises = { tags = ["eyes", "neck", "shoulders", "wrists"] }`,
  new `move-break` every 30 min for 2 min (toast, tags walk/legs/hips),
  `rest-break` every 60 min for 10 min with the full planner; `[exercises]`
  with `enabled`, `equipment`, `exclude`, `walks`. Existing installs keep
  their own config.toml.
- **Catalogue** `config/exercises.toml`: 28 items (14 seated micro items,
  14 standing; stairs, two band and two floor items behind equipment
  flags), each with tags, posture, seconds, reps, a toast cue, 1-4 overlay
  lines, an evidence grade and the reason with its source. Built in with
  `include_str!`; `<data dir>/exercises.toml` replaces or adds by id,
  `enabled = false` hides.
- **ns-core**: `config::Exercises` and `Equipment`; `Routine` and
  `RoutineStep`; `NagEvent::Raise.routine` and `BreakStarted.routine`;
  `LocalDay::days_since`.
- **ns-rules** `exercises/`: `Catalog` (parse, validate, merge),
  `planner::plan` (one item under a minute; a walk scaled 1 to 5 min by the
  time sat and capped at 60% of the budget, region-balanced items, breathing
  last in long routines; weights by days since shown, grade A, day load,
  the rule's previous items; seeded `SmallRng`), `Rotation` (JSON in
  `settings` as `exercises.rotation`). `Rule.exercises` (`false`, `true` or
  `{ tags }`). The engine plans when a break rule's nag is raised, keeps
  the routine through a snooze, reuses it at `StartBreakNow` (plans fresh
  for a tray start) and notes it in the rotation; `RuleSnapshot.routine`;
  `Engine::with_exercises`, `set_exercises_config`, `import_rotation`,
  `export_rotation`; `EngineTask` persists `EngineState { counters,
  rotation }`; `EngineCommand::SetExercises`. 14 new tests (catalogue
  validity and merge, budgets, walk scaling, balance, flags, rotation,
  determinism, day load).
- **ns-storage**: migration 003 adds `nag_events.routine`; `log_nag` and
  `nags` carry it.
- **ns-api**: `GET /v1/exercises` (`{exercises, config}`); `/nags` rows
  carry the routine as JSON; `ApiState.catalog`.
- **ns-app**: the runtime loads the catalogue (built-in plus the user file,
  a broken file is logged and ignored), restores the rotation, saves it
  with the counters, sends `SetExercises` on every config save, and logs
  routines with raise and break-start events; `NagView.routine`.
- **UI**: the overlay shows the routine next to the ring (step number,
  name, reps, lines, a per-step countdown, Next, the full list with done
  and current steps, the note, "Not medical advice"); Space advances; the
  toast shows "Try/First: <name> · <reps>" and the cue; the overlay nag
  lists the planned items. Settings > Break exercises: on/off, walks,
  equipment checkboxes, "All 28 exercises" with per-item switches,
  expandable steps and reasons (edits `[exercises]` in the config text via
  `lib/toml.ts` `readExercises` / `withExercises`). Rules cards get an
  exercises chip; the nag history shows each routine's items.
- Docs: ARCHITECTURE.md "Break exercises", MANUAL_CHECKS.md "Break
  exercises", README, config comments, EXERCISES.md "Built:" notes;
  `tools/seed_demo.py` seeds routines.
- Verified: `cargo test --workspace` (115 tests), clippy `-D warnings`,
  fmt, `npm run ui:check`, the Linux cross-check; the Tauri debug build on
  the hidden desktop (`--no-tray`, scratch data dir): migrated to version
  3, `/v1/exercises` listed 28 items, `POST /control/break/start` for
  `rest-break` produced a six-step routine (walk 60 s first, cyclic
  sighing last, 260 s of a 540 s budget) that `/state` and `/nags` both
  carried, and the overlay rendered on both monitors with the step view.
- Issues: the planner's `candidates` closure and a test config needed
  clippy's shapes; the catalogue test first assumed a free cardio item
  (stairs is the only one) and 20 free items (23). A PowerShell job could
  not quote an inline Python one-liner; the token lookup moved to a file.

## 2026-11-04_223500_1793831700: Break exercises: third default rule decided

- docs/EXERCISES.md "Default rules": `move-break` (every 30 min, 2 min,
  toast, tags walk/legs/hips) joins the shipped rules; `micro-break` goes
  from every 10 min to every 20 min and `rest-break` from 50 to 60 min so
  the three line up (light activity every 30 min, a longer rest every
  hour) and rarely fire within minutes of each other. Still a plan: the
  config change is work item 0 of the brief, nothing in `config/default.toml`
  changed yet.

## 2026-11-04_222508_1793831108: Break exercises design brief

- docs/EXERCISES.md: what to show during a break instead of "Stand up,
  look away, stretch." Written after a literature pass (sources listed in
  the brief): light walking beats standing for glucose and insulin
  (meta-analysis), 5 min every 30 min is the dose that also lowers blood
  pressure, 3 min of squats matches 3 min of walking, strength work has
  the best evidence for neck and shoulder pain, stretches and the
  20-20-20 rule help symptoms in small trials, cyclic sighing has one RCT.
  The brief grades every item, excludes the hyped or unsupported ones
  (soleus push-ups, palming, neck circles, nerve gliding for prevention),
  and specifies a 28-item catalogue in `config/exercises.toml`, a pure
  planner and rotation in `ns-rules`, the engine, storage, API and UI
  hooks, work items with acceptance checks, and five open questions.
- No code changed. PLAN.md links the brief under "Next steps".

## 2026-11-04_140801_1793797681: Browser extension 0.2 (docs/EXTENSION_V2.md)

- **Tracking edges** (`background.ts`): a focused window with no readable
  active tab (a private window the extension may not run in) records
  `focus_lost`, so the previous tab stops accruing. One heartbeat and flush
  at every worker start (`runtime.onInstalled`/`onStartup` listened to as
  well). The flush backoff and retry deadline live in `storage.session`, so
  a restarted MV3 worker does not probe a closed app every 30 s; the Chrome
  manifest sets `minimum_chrome_version` 102.
- **Port discovery** (`api.ts`): the remembered port first, then the other
  three in parallel, 1 s timeout each: "not running" within about 2 s
  instead of 6 s. The last good port is reused for the worker's lifetime and
  re-probed only after a connection failure, which drops a `/health` round
  trip per flush.
- **Toolbar popup** (`popup.html`, `popup.ts`, `format.ts`): status, the
  last reported site and whether it is counted (Counting / Browser not in
  front / Idle / Tracking paused), today's web time with the top five hosts,
  the enabled rules' progress, Snooze/Skip for a due or nagging rule, Start
  break now, Pause 30 min / Resume. Built from `/state` and `/stats/web` as
  they are; no host change. Optional badge with today's web time (off by
  default). Colors from the dashboard's tokens, light and dark
  (`common.css`). `action.default_popup` is `popup.html` now.
- **Options page**: badge toggle, "Forget everything" (two clicks: embedded
  options pages show no `confirm()`; clears `storage.local` and
  `storage.session`), the hint where to allow private windows when the
  checkbox is on but `isAllowedIncognitoAccess()` is false, and a Privacy
  link to `privacy.html`, which the build renders from
  `extension/store/privacy.md`.
- **Tests**: vitest 5 with a fake WebExtension API (`extension/test/`), 39
  cases (queue, port probing, 401, pairing, browser families, start-up
  heartbeat, `focus_lost`, unfocused `tab_updated`, incognito scrub,
  retry deadline across a simulated worker restart, badge, popup helpers).
  `npm run ext:test`, also in CI after `ext:check`.
- **Store readiness**: `ext:check` runs tsc, the build and `web-ext lint`
  (0 errors, 0 warnings). Lint flagged the missing
  `data_collection_permissions` that AMO requires of new add-ons; the
  Firefox manifest declares `browsingActivity` (url and title leave the
  extension, if only for 127.0.0.1), and because that key needs Firefox 140,
  **`strict_min_version` went from 115 to 140** (ESR), with
  `gecko_android` 142 for lint. `npm run ext:source` zips the sources for
  AMO (reusing `zip()` from `package.mjs`), built in the release workflow.
  `extension/store/`: listing, permissions, privacy and screenshot texts.
  docs/RELEASING.md: what to paste where, Edge takes the Chrome zip.
- **Harness**: `tools/ext_e2e.ps1 -Browser edge` also opens
  `chrome-extension://<id>/popup.html` over DevTools
  (`tools/ext_popup_check.mjs`, Node's global `WebSocket`), seeds one host
  row for today (no web time accrues on the hidden desktop), checks the
  paired status and the host, and saves `%TEMP%an-ext-e2epopup-edge.png`.
  Edge and Firefox both PASS. The start-up heartbeat cannot be seen there:
  the harness's copy pairs itself after the worker has started.
- Issues: a Python heredoc through Git Bash hung on the Store `python`
  stub (stopped, edited with the editor instead). The extension version
  stays 0.1.0 because the release workflow requires one version for every
  package.

## 2026-11-04_133502_1793795702: Extension 0.2 build brief

- docs/EXTENSION_V2.md: a self-contained brief for the next extension
  version, written so an implementation session can take it as its whole
  task. It records what the 0.1 extension already does and the host API it
  uses, then lists the work: `focus_lost` when the focused window cannot be
  read (private windows without permission keep the last tab accruing
  today), a heartbeat at start-up, the retry deadline in `storage.session`
  across service-worker restarts, parallel port probing, a toolbar popup
  separate from the options page, options additions (badge toggle, "Forget
  everything", the private-window permission hint), vitest tests with a
  fake WebExtension API, `web-ext lint` in `ext:check`, an AMO source zip,
  store listing texts, and a popup screenshot in `tools/ext_e2e.ps1`.
- No code changed. PLAN.md links the brief under "Next steps".

## 2026-11-04_110045_1793790045: License change to PolyForm Noncommercial

- `LICENSE.md` holds PolyForm Noncommercial 1.0.0 (verbatim from
  polyformproject/polyform-licenses tag 1.0.0); `LICENSE-MIT` and
  `LICENSE-APACHE` are gone. D25 records the reasoning: personal and
  non-profit use stays free, commercial use needs the author's permission,
  and copies must carry the terms plus the `Required Notice:` line now in
  README.md.
- SPDX id `PolyForm-Noncommercial-1.0.0` in the workspace `Cargo.toml` and
  root `package.json`; the Settings "About" card, README and PLAN say
  "source-available" instead of "open source".
- Commits before this one were published under MIT OR Apache-2.0 and remain
  so for anyone who already copied them.

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
