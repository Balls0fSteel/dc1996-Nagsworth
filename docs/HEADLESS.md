# Headless-first: a tracking service with the GUI as its official client

Drafted 2026-11-04_232332_1793834612. Status: proposal, nothing built. The
open questions at the end need the owner's answers before work starts.
Read AGENTS.md, PLAN.md, docs/ARCHITECTURE.md and docs/DECISIONS.md (D2)
first.

## Why

D2 kept a split possible ("the composition root uses no Tauri types") but
shipped one tray process. The owner asked whether it would make more sense
to build headless-first and make the GUI the "official viewer". It does,
for four reasons:

1. **Most of it is already a client.** The dashboard reads everything over
   `/v1` with a bearer token (`ui/src/lib/api.ts`); the only Tauri commands
   it uses are `get_api_info` and the autostart pair. The browser extension
   is a second client of the same API. The parts that are not clients yet
   are few: the nag windows (fed by `nag_bridge.rs` events, not the API),
   the tray, autostart, and the updater.
2. **Footprint.** Today the WebView2 (or WebKitGTK) runtime stays loaded
   for the whole session, because the hidden dashboard window and the
   bridge live in the same process as the tracker. A service that only
   samples, writes SQLite and serves the API is a few MB; the GUI can be
   closed for real and only start when there is something to show.
3. **Other clients become first-class.** A waybar/polybar module, a
   Rainmeter skin, a CLI (`nagsworth status`, `nagsworth break`), a phone on
   the LAN later: all talk to the same API the official GUI uses, so the API
   cannot drift from what the GUI needs.
4. **Robustness.** A crash or hang in a webview (driver, GPU, a Svelte bug)
   no longer stops tracking or loses the open segment; the service keeps
   counting and the GUI reconnects.

What it does not change: tracking still needs the user's desktop session
(foreground window, idle time, lock events), so the service is a
**per-user background process**, never a Windows service in session 0 or a
system-wide systemd unit. And somebody still has to draw the overlay: a
break overlay needs a window, so "headless" means "the core runs without
the GUI", not "nags without any UI".

## Target shape

```
nagsworthd  (per-user, autostarted)            nagsworth  (GUI, Tauri)
├─ ns-runtime  (today's runtime.rs)            ├─ dashboard window   ─┐
│  sampler, segmenter, web, engine,            ├─ toast / overlays    ├─ /v1 + SSE
│  storage, side effects, retention            ├─ tray (see Q1)      ─┘
├─ ns-api on 127.0.0.1:47821/v1                └─ pair window
├─ lock nag (LockWorkStation), beeps
├─ updater (ns-update)                         other clients: extension,
└─ fallback notifications (see Q2)             CLI, bar widgets
```

- **`ns-runtime`** (new crate): `src-tauri/src/runtime.rs` moves here
  unchanged in spirit (it already has no Tauri types), plus `side_effects`
  and `retention`. `Runtime::start(data_dir, platform)` stays the one
  entry point.
- **`nagsworthd`** (new bin, `crates/ns-daemon` or `src-daemon/`): parses
  flags, takes the single-instance lock, starts `ns-runtime`, writes the
  discovery file, waits for a shutdown signal (Ctrl-C, `WM_ENDSESSION` /
  `SIGTERM`, or `POST /v1/shutdown`), saves rule counters and closes the
  open segment as `Runtime::shutdown` does now.
- **`nagsworth`** (the Tauri app): no tracking code. On start it reads the
  discovery file; if the service is not running it starts `nagsworthd`
  (same install folder) and waits up to 5 s for `/health`. It subscribes
  to `/v1/events` (SSE) and drives the toast and overlays from it.

### What has to move into the API

| Today (Tauri only) | Becomes |
|---|---|
| `nag_bridge.rs` turns `NagEvent`s into `NagView`s and `nags-changed` | The bridge logic (one countdown, demote other breaks, toast vs overlay, exclusive-fullscreen deferral) moves into `ns-runtime` as a pure `NagBoard`; `GET /v1/nags/current` returns the views, and SSE sends `nags_changed` with the same payload. The GUI only opens and closes windows. |
| `current_nags`, `nag_action` commands | `GET /v1/nags/current`; the existing `/v1/control/{snooze,skip,break/start}`. |
| `nag_prefs` (step tick) | Part of `/v1/exercises` `config` (already there). |
| `get_api_info` | The discovery file (below). |
| `get_autostart`, `set_autostart` | `GET/PUT /v1/autostart`, served by the service, which registers **itself** (the GUI is no longer what logs in with the user). |
| Updater commands (`update_*`) | `/v1/update/{status,check,download,install,dismiss,settings}` in the service; the GUI's update banner reads them. The service runs the setup and exits; the setup restarts the service with `--hidden` (D22 unchanged otherwise). |
| `tray.rs` | See Q1. |
| `SHQueryUserNotificationState` deferral | Stays in `ns-platform`, called by the service's `NagBoard`; the view carries `deferred: true` and the service beeps, as now. |

### Discovery and auth

`<data dir>/api.json`, written by the service at start and removed at clean
exit: `{ "version": 1, "pid": …, "base_url": "http://127.0.0.1:47821/v1",
"token": "…" }`. The data directory is already per-user; on Windows the
file gets an ACL for the current user only, on Linux mode 0600. Any local
client run by the same user reads it, which is the same trust boundary as
reading `activity.db` today. The pairing flow (`/v1/pair`) stays for the
browser extension, which cannot read files. A stale file (pid not running,
`/health` fails) means "not running".

### Process lifetime

- **Single instance**: a named mutex on Windows
  (`Local\nagsworth-daemon-<user sid>`), a lock file with `flock` in
  `$XDG_RUNTIME_DIR` on Linux. A second `nagsworthd` exits 0 after
  printing the running instance's URL.
- **Autostart**: the service's Run key / XDG autostart entry (the platform
  code already exists, it just points at the other binary). Linux may also
  ship a `systemd --user` unit (`nagsworth.service`, `PartOf=graphical-
  session.target`) for users who prefer it; the autostart entry stays the
  default because not every desktop starts the graphical-session target.
- **GUI quit** no longer stops tracking. The tray (if the GUI owns it) gets
  "Quit window" and "Stop tracking and quit" (`POST /v1/shutdown`).

## Nags without the GUI (Q2)

When no GUI client is subscribed to the SSE stream, a nag still has to
reach the user. Options:

- **A. The service starts the GUI** in nag-only mode (`nagsworth --nag`),
  which opens the toast or overlay and exits when the nag is gone. Same
  windows as today; costs a webview start (about 0.5-1.5 s on Windows) per
  nag after the GUI was closed.
- **B. OS notifications**: a Windows toast (needs the installer's AUMID
  shortcut, which NSIS already creates) and `org.freedesktop.Notifications`
  on Linux, with "Start break" / "Snooze" actions where the platform
  supports them. Overlay-level nags fall back to A or to a beep plus an
  ever-present notification.
- **C. Both**: B for toast level, A for overlay level.

Proposal: **C**. Toast-level nags are exactly what OS notifications are
for, and the overlay must be our own window anyway.

## Phases

Each phase ships on its own and leaves the app working.

1. **Extract `ns-runtime`** (pure move): `runtime.rs`, side effects and
   retention into a crate; `src-tauri` depends on it. No behaviour change.
   Tests: the existing ones, plus `Runtime::start` on a temp dir with
   `MockPlatform` (new: proves it needs no Tauri).
2. **Nag board into the runtime + API**: `NagBoard` (pure, the bridge's
   logic and its tests), `GET /v1/nags/current`, `nags_changed` on SSE,
   autostart and update endpoints. The GUI switches its nag windows, update
   banner and autostart switch to the API while still hosting the runtime
   in-process. This is the phase that makes the GUI a client.
3. **`nagsworthd`** binary, discovery file, single-instance lock,
   `POST /v1/shutdown`, the GUI starting the service when absent. The
   installer (NSIS hooks, `.deb`) ships both; autostart registers the
   service. Upgrade path: the first start of the new GUI moves the Run key
   from `nagsworth.exe --hidden` to `nagsworthd.exe`.
4. **Nags without the GUI** (Q2), the `--nag` mode and/or OS notifications.
5. **CLI and docs**: `nagsworth-cli` (or `nagsworthd status|break|pause`),
   docs/API.md as the public contract with a version field (`/v1` is
   already versioned; add `/v1/health` `api_version`), an example bar
   module in `tools/`.

Rough size: phase 1 is small (a day), phase 2 is the bulk (the bridge,
three endpoint groups, UI rewiring), phase 3 is medium (process plumbing
and both installers), 4 and 5 are independent and can wait.

## Tests and checks

- Unit: `NagBoard` (ported bridge tests), discovery file round trip and
  stale detection, single-instance lock, the new endpoints in
  `crates/ns-api/src/tests.rs`.
- Hidden-desktop runs (AGENTS.md): start `nagsworthd` alone, drive a break
  over the API, then start the GUI and see it pick up the running break;
  kill the GUI during a break and check the service keeps counting and the
  break ends on time; kill the service and check the GUI shows "service not
  running" and restarts it.
- `tools/windows_install_test.ps1`: both binaries installed, the Run key
  points at the service, an update replaces both and restarts the service.
- `tools/linux_checks.sh` / `linux_app_check.sh`: the service under Xvfb
  and headless sway without the GUI; the `.deb` installs both.

## Risks

- **Two processes to update atomically.** The setup must stop the GUI and
  the service; D22's `/WAITPID` waits for one pid. The service owns the
  update, so it asks the GUI to exit (SSE `shutdown` event) before starting
  the setup and passes its own pid.
- **Version skew** between a running service and a newer GUI after a
  partial update: `/v1/health` reports the service version; the GUI refuses
  to drive a service with a different major API version and offers a
  restart.
- **Port in use**: unchanged from today (the API falls back to "not
  running"); with a discovery file the service could also pick a free port,
  but the extension pairs to 47821, so keep the fixed port and report the
  conflict.
- **Webview start latency** for overlay nags when the GUI was closed
  (Q2 option A); measure before deciding.
- **Linux tray**: if the service owns the tray (Q1), it links GTK /
  libayatana-appindicator, so `nagsworthd` is not usable on a machine
  without them; a `--no-tray` build feature keeps a truly headless variant.

## Open questions for the owner

1. **Who owns the tray?** (a) The service, through the `tray-icon` crate
   (no webview; the menu opens the GUI), so the tray is there whenever
   tracking runs and the GUI can be fully closed. (b) The GUI, which then
   stays resident with no windows open (WebView runtime unloaded only if
   every webview is destroyed). Proposal: (a).
2. **Nags without the GUI**: A (start the GUI per nag), B (OS
   notifications) or C (B for toasts, A for overlays). Proposal: C.
3. **One installer or two?** Proposal: one installer with both binaries;
   a headless-only Linux tarball later if anyone asks.
4. **Binary names**: `nagsworthd` + `nagsworth`, or `nagsworth` (service)
   + `nagsworth-ui`? Proposal: keep `nagsworth.exe` for the GUI (it is
   what users click and what the shortcut points at) and add `nagsworthd`.
5. **Order against other work**: before or after the v0.1.0 tag and the
   store submission (PLAN.md next steps 4 and 4a)? Proposal: tag 0.1.0
   first as the single-process build; phases 1-2 next, since they change
   nothing for users; phase 3 for 0.2.0.
