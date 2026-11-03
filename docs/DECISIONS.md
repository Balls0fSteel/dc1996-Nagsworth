# Decisions

Owner-approved on 2026-10-31 (D1-D8), and implementation decisions made
since (D9 onward).

| ID | Decision |
|---|---|
| D1 | Windows first, Linux later. A `Platform` trait isolates OS code so an X11/Wayland backend can be added. |
| D2 | Rust + Tauri 2. One tray process built from workspace crates; a headless service split stays possible because the composition root (`runtime.rs`) uses no Tauri types. |
| D3 | SQLite via `rusqlite` (`bundled`) + `rusqlite_migration`, one storage actor thread owning the connection. |
| D4 | Frontend: Svelte 5 + TypeScript + Vite, multi-page (`index`, `toast`, `overlay`, `pair`). |
| D5 | Browser extension (MV3, Chrome + Firefox) talks to a local HTTP API (`axum`) on `127.0.0.1:47821` with a bearer token, with a short port fallback list the extension probes. |
| D6 | No global hooks, no injection, no reading other processes' memory. Idle via `GetLastInputInfo`; Steam via `HKCU\Software\Valve\Steam\RunningAppID`; the `lock` level calls `LockWorkStation()`. |
| D7 | Window titles stored by default (`tracking.store_titles`); `lock` level supported but commented out in shipped rules; one DB per Windows user under `%LOCALAPPDATA%\activity_nagger\`; license MIT OR Apache-2.0. |
| D8 | Tauri CLI is a project devDependency (`npm run tauri`). |
| D9 | Event payload types (`TrackerEvent`, `NagEvent`, `ControlCommand`, ...) live in `an-core`, not in the tracker/rules crates, so the shared `Event` enum needs no dependency cycle. |
| D10 | Daily totals are updated incrementally on every segment extension (not only on close), so a crash loses at most one flush interval and live stats include the open segment. |
| D11 | Apps are identified by exe name (case-insensitive), not by path, so updates that move the install folder (`app-1.2.3\`) keep one app. |
| D12 | Rules match categories by kind (Work, Game, Web, Other); custom categories carry a kind. |
| D13 | The rules live in the config file; `GET/PUT /rules` read and write the whole file, validated before saving and hot-reloaded. Tracking settings such as `poll_ms` and the API port need a restart. |
| D14 | Escalation pauses while the user rests (locked, idle under `pause`), so stepping away never escalates a nag to a lock. |
| D15 | Nag windows read state through Tauri commands/events; the dashboard uses the HTTP API with the token from `get_api_info`. One data path for the dashboard and the extension. |
| D16 | The extension bundles each script as a self-contained IIFE so Chrome's service worker and Firefox's background scripts load the same file without module support. |
| D17 | Linux: Wayland protocols first (ext-idle-notify, wlr-foreign-toplevel), X11/XWayland as fallback, logind for session events and lock. No GNOME Shell extension; GNOME Wayland tracks X clients only. |
| D18 | Exe names are compared by stem (lower case, no `.exe`), so one set of lists works on Windows and Linux. |
| D19 | During exclusive fullscreen, overlays are deferred (beep + tray tooltip) rather than forced on top, in line with D6. |
| D20 | Signing details stay out of `tauri.conf.json`; CI merges a generated `tauri.signing.json` when certificate secrets exist. |
| D21 | Users extend game detection with `known_games.toml` in the data directory; Steam games are named from `appmanifest_<id>.acf`. |
