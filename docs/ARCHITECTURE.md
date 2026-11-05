# Architecture

One tray process. Tracking, rules and the local API run on a dedicated
tokio runtime started by the `ns-runtime` crate (`Runtime::start`), which
has no UI dependency; Tauri only hosts the tray and the webview windows. A
per-user `nagsworthd` service will host the same crate (docs/HEADLESS.md).

```
 platform thread (WTS + power notifications)
        | SessionEvent                                   EventBus (broadcast<Event>)
        v                                                     ^   ^   ^   |
 Sampler --mpsc<Sample>--> SegmenterTask ------- Tracker -----+   |   |   +--> side_effects: nag log, LockWorkStation,
   (1 s poll)               | Segmenter (pure)                    |   |        Reload, reading-mode setting
                            | Categorizer                         |   |   +--> nag_bridge (Tauri): toast / overlays
                            v                                     |   |   +--> API /events (SSE)
                     StorageHandle (actor thread, SQLite)         |   |
                            ^                                     |   |
 extension --HTTP--> ns-api --mpsc<ExtEvent>--> WebTask -- Web ---+   |
                       |        (WebSegmenter, reads activity)        |
                       +-- Control commands --------------------------+
                                                                      |
 watch<Activity> (segmenter) --> EngineTask (Engine, 1 s tick) -- Nag +
                                    | watch<EngineSnapshot> --> API /state, tray, UI
```

## Tracking

- `Sampler` polls `Platform::sample()` every `tracking.poll_ms` on a blocking
  thread: foreground window (title, process, fullscreen, monitor), idle time
  (the smaller of `GetLastInputInfo` and the time since the last XInput
  controller change, see docs/HANDS_ON_TIME.md), and session state from
  WTS/power notifications.
  UWP apps hosted by `ApplicationFrameHost.exe` are resolved to the child
  window's process. Process info is cached by pid + creation time.
- `Segmenter` (pure) turns samples into open/extend/close actions:
  - a key change (kind, exe, title) starts a new segment; active-to-active
    switches must last `min_segment_ms` or they merge into the current one;
  - going idle is backdated to the last input;
  - a gap longer than the poll interval + 5 s becomes a `suspended` segment;
  - segments split at the local day start (`general.day_start`);
  - the open segment is flushed every 10 s;
  - hands-on time: a sample whose idle time is within
    `tracking.input_window_s` credits the time since the previous sample to
    the active segment (`input_ms`), so "in front" and "hands-on" can be
    compared per segment, app and day.
- `SegmenterTask` maps actions to storage writes and bus events, looks up
  (or creates) the app with its category, and publishes the current
  `Activity` on a watch channel after every sample.
- `Categorizer` picks the default category for a new app: known game exes >
  Steam (`RunningAppID` + `steamapps\common`) > launcher folders > browsers /
  common work apps > Other. A category set by the user is never overwritten.
- `WebSegmenter` keeps each browser's active tab from extension events and
  accrues web time only while that browser is the foreground app and the
  activity is not idle. With `web.store_url = "host"` the API strips URLs to
  the origin before they reach the tracker.

### Linux backend

`ns-platform/src/linux/`: on Wayland a thread binds `ext-idle-notify-v1`
(idle) and `zwlr_foreign_toplevel_manager_v1` (focused window; the app id
stands in for the exe, there is no pid). Without those it falls back to X11
via x11rb: `_NET_ACTIVE_WINDOW`, `_NET_WM_NAME`, `_NET_WM_PID` to
`/proc/<pid>/exe`, `_NET_WM_STATE_FULLSCREEN`, and MIT-SCREEN-SAVER for idle.
logind supplies Lock/Unlock and PrepareForSleep over the system bus and
`Session.Lock` for the `lock` level. Autostart is an XDG `.desktop` file;
Steam state comes from `~/.steam/registry.vdf`. Exe names are compared
without `.exe` so the same lists categorize both systems.

## Storage

`ns-storage` owns one connection on an actor thread; callers send closures
(`StorageHandle::call`, `send`, `call_blocking`). WAL, `synchronous=NORMAL`,
migrations via `rusqlite_migration`. Tables: `categories`, `apps`, `titles`,
`segments`, `web_visits`, `ext_events_seen` (dedup), `daily_app_totals`,
`daily_kind_totals`, `daily_web_totals`, `nag_events`, `rule_state`,
`settings`. Migration 002 adds `input_ms` (hands-on milliseconds) to
`segments` and both daily total tables. `GET /export` returns segments (CSV or JSON, with web visits) for
a day range; `POST /backup` writes a `VACUUM INTO` copy to
`<data dir>/backups/`. Totals grow with every segment extension, so reports never scan
segments (except the one-day timeline). Retention prunes segments, visits
and nag history older than `tracking.retention_days`; totals stay.

## Updates

`ns-update` is pure apart from HTTP and the registry: `Version` (semver
order), `Manifest` (the newest release and pre-release from GitHub's
releases JSON), `Checker` (when to check, what to show, which banner was
dismissed), `download_setup` (the setup and `SHA256SUMS`, hash-checked,
written as `.part` and renamed) and, on Windows, `installed` (the Settings >
Apps entry names this folder) and `run_setup` (the file locked and re-hashed,
then `/S /UPDATE /R /WAITPID=<pid> /ARGS --hidden`). `ns_runtime::update`
owns the state, runs the timer and the downloads on threads, persists the
three `update.*` settings and publishes `Event::Update` on the bus; the API
serves it as `GET /update` (actions under `/update/*`) and streams it, the
dashboard's banner and Settings card follow that, and the tray reads the
`Updater` in-process. Starting the setup publishes `AppEvent::ExitForUpdate`,
on which the app exits.

## Rules

`Engine` is pure: `tick(now, offset, activity)` and `command(cmd, now)` return
`NagEvent`s. Per rule it keeps counters (`accum_ms`, `rest_ms`, snoozes, day)
and a `NagState`:

```
Idle --due--> Active{level} --escalation--> Active{higher}
Active --snooze--> Snoozed{until} --until passed, still due--> Active
Active --skip--> Idle (break rules) | Satisfied{since} (limits, schedules)
Active/any --start break--> InBreak{until} --until passed--> Idle
Active --enough rest--> Idle;  --left scope--> Satisfied (daily) | Idle (schedule)
Satisfied --renag interval passed, still due--> Active
```

- Counting time: the activity is in the rule's scope and counts under the
  rule's idle policy. `pause`: only `active` counts. `ignore`: `active` and
  `idle` count (time in front of the screen). `input`: `active` or `idle`
  counts only while the activity is hands-on (`Activity.hands_on`: the last
  input on any device, gamepads included, at most `tracking.input_window_s`
  ago). Without an explicit `idle_policy`, Game category rules default to
  `input` while `tracking.hands_on_games` is on (the default) and to `ignore`
  when it is off; all other rules default to `pause`. Reading mode turns
  `pause` into `ignore` and leaves `input` alone. The effective policy is in
  each rule's snapshot (`idle_policy`) and the toggle in
  `engine.hands_on_games` of `GET /state`.
- Rest: everything else, including lock, sleep and paused tracking. Gaps
  between ticks longer than 5 s are rest.
- Escalation follows the rule's steps by time since the nag was raised, and
  only advances while the user keeps going.
- Counters are persisted every minute and on shutdown (`rule_state`), so a
  restart does not reset a daily limit.

### Break exercises

`ns_rules::exercises` (docs/EXERCISES.md) is pure like the engine. `Catalog`
parses `config/exercises.toml` (built in) and merges the user's
`<data dir>/exercises.toml` by id. `planner::plan` builds a `Routine` from
the rule's `break_for`, the time counted since the rule's last reset
(`sat_ms`), today's active time and the rule's `exercises` tags: one item
under a minute; a walk whose length follows the time sat (1 to 5 min) plus
region-balanced items for longer breaks; breathing last in long untargeted
routines. `Rotation` (which item was shown when, per rule) weights the
seeded choice and is persisted in `settings` as `exercises.rotation`. The
engine plans when a break rule's nag is raised (the toast shows the first
item's cue) and reuses that routine when the break starts, or plans then for
a break started from the tray. `NagEvent::Raise` and `BreakStarted` carry the
routine; `nag_events.routine` logs it; `GET /exercises` lists the catalogue
and `[exercises]` for the Settings card, whose changes reach the engine as
`EngineCommand::SetExercises`. The catalogue file is read at start only.
Items tagged `habits` (water, daylight) appear only with `[exercises]
habits`. `SkipBreak` during a running break ends it with
`DismissReason::EndedEarly`; `GET /stats/breaks` pairs each `break_started`
row with the next dismiss of its rule (by row id) and counts started,
completed (`break_taken`), ended early and breaks with a routine per local
day, for the History "Breaks" card. The overlay's step tick
(`[exercises] step_sound`, read from `GET /exercises`) plays in
the `overlay-0` window only.

## Nag windows

`ns_runtime::nag_board` keeps the open nags from the bus (seeded from the
engine snapshot at start) and decides the windows: a toast (bottom-right of
the primary work area, does not take focus) for toast-level nags, and one
undecorated, always-on-top overlay per monitor for overlay/lock-level nags
and running breaks. It publishes a `NagsView` (nags plus `toast`,
`overlay`, `deferred`) on a watch channel and as `Event::Nags`.
`src-tauri/src/nag_bridge.rs` only opens and closes the windows to match.
The pages read `GET /nags/current`, follow the `nags` events on
`/events?sources=nags` (fetch streaming, since EventSource cannot send the
token), and answer with `/control/{snooze,skip,break/start}`. During a break the overlay walks the
routine's steps on their own seconds (Space or "Next" jumps ahead); the
engine's break timer stays the truth and the steps are only a view on it. The `lock` level is applied by the
runtime (`LockWorkStation`), not by a window.

While an exclusive-fullscreen game or a presentation owns the screen
(`SHQueryUserNotificationState`), topmost windows would stay invisible, so
overlays wait: the board beeps once, the tray tooltip names the nag, and the
overlay opens as soon as the app leaves exclusive mode (rechecked every
second).

## API

`http://127.0.0.1:47821/v1` (falls back to 47822-47824). Bearer token from
`settings.api.token`, generated on first start; `/health` and `/pair` are
public. CORS allows extension origins, the Tauri origin and localhost dev
servers. Routes: `/health`, `/pair`, `/pair/start`, `/ext/events`,
`/ext/heartbeat`, `/state`, `/stats/{apps,categories,daily,kinds,timeline,web}`,
`/apps`, `/apps/{id}`, `/categories`, `/rules` (GET/PUT TOML), `/nags`, `/export`, `/backup`,
`/events` (SSE, `?token=` allowed), `/control/{snooze,skip,break/start,pause,resume,reading-mode}`.

## Extension

MV3, one codebase for Chrome (also Edge, Brave, Opera, Vivaldi; Chrome 102+)
and Firefox (140+). `background.ts` listens to tab activation/updates/removal,
window focus and idle changes; events go into a `storage.local` FIFO (cap
5000) that is flushed at 20 items and by a 30 s alarm, with backoff (5 s
doubling to 5 min) when the app is down. The retry deadline lives in
`storage.session`, so a service worker Chrome stopped and restarted keeps
waiting. A focused window whose active tab cannot be read (a private window
the extension may not run in) is reported as `focus_lost`. A heartbeat
reports the active tab at every worker start and then every minute.
Incognito tabs are reported only as "no tab" unless the user opts in.

`api.ts` finds the app by probing the remembered port, then 47821-47824 in
parallel (1 s each, so "not running" within about 2 s), and reuses the last
good port until a connection fails. Pairing: the dashboard shows a 6-digit
code (5 min, one use, burned after 5 wrong guesses) that the options page
exchanges for the token.

The toolbar popup (`popup.ts`) is built from `GET /state` and
`GET /stats/web` as they are: status, whether the last reported tab is being
counted, today's web time and top hosts, rule progress, and the tray menu's
controls (`/control/*`). An optional badge shows today's web time. The
options page holds pairing, the badge and private-tab settings, "Forget
everything" and the privacy text, which `scripts/build.mjs` renders from
`extension/store/privacy.md`. Pure helpers are in `format.ts`; unit tests
in `extension/test/` run the modules against a fake WebExtension API
(`npm run ext:test`).
