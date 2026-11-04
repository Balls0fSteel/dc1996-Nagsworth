# Architecture

One tray process. Tracking, rules and the local API run on a dedicated
tokio runtime started by `src-tauri/src/runtime.rs`, which uses no Tauri
types; Tauri only hosts the tray and the webview windows.

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
  from `GetLastInputInfo`, and session state from WTS/power notifications.
  UWP apps hosted by `ApplicationFrameHost.exe` are resolved to the child
  window's process. Process info is cached by pid + creation time.
- `Segmenter` (pure) turns samples into open/extend/close actions:
  - a key change (kind, exe, title) starts a new segment; active-to-active
    switches must last `min_segment_ms` or they merge into the current one;
  - going idle is backdated to the last input;
  - a gap longer than the poll interval + 5 s becomes a `suspended` segment;
  - segments split at the local day start (`general.day_start`);
  - the open segment is flushed every 10 s.
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
`settings`. `GET /export` returns segments (CSV or JSON, with web visits) for
a day range; `POST /backup` writes a `VACUUM INTO` copy to
`<data dir>/backups/`. Totals grow with every segment extension, so reports never scan
segments (except the one-day timeline). Retention prunes segments, visits
and nag history older than `tracking.retention_days`; totals stay.

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

- Counting time: the activity is in the rule's scope and `active`, or `idle`
  under idle policy `ignore` (reading mode forces `ignore`; the Game category
  defaults to it because gamepad input is invisible to `GetLastInputInfo`).
- Rest: everything else, including lock, sleep and paused tracking. Gaps
  between ticks longer than 5 s are rest.
- Escalation follows the rule's steps by time since the nag was raised, and
  only advances while the user keeps going.
- Counters are persisted every minute and on shutdown (`rule_state`), so a
  restart does not reset a daily limit.

## Nag windows

`nag_bridge.rs` keeps the open nags from the bus and reconciles windows: a
toast (bottom-right of the primary work area, does not take focus) for
toast-level nags, and one undecorated, always-on-top overlay per monitor for
overlay/lock-level nags and running breaks. Windows fetch the list with the
`current_nags` command, follow `nags-changed` events, and answer with
`nag_action` (snooze, skip, break). The `lock` level is applied by the
runtime (`LockWorkStation`), not by a window.

While an exclusive-fullscreen game or a presentation owns the screen
(`SHQueryUserNotificationState`), topmost windows would stay invisible, so
overlays wait: the bridge beeps once, the tray tooltip names the nag, and the
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

MV3, one codebase for Chrome and Firefox. `background.ts` listens to tab
activation/updates/removal, window focus and idle changes; events go into a
`storage.local` FIFO (cap 5000) that is flushed at 20 items and by a 30 s
alarm, with backoff when the app is down. A 1 min heartbeat reports the
active tab. Incognito tabs are reported only as "no tab" unless the user opts
in. Pairing: the dashboard shows a 6-digit code (5 min, one use, burned
after 5 wrong guesses) that the options page exchanges for the token.
