# Manual checks

What automated runs on the development PC cannot cover. Each check says how
to run it and what passing looks like. Logs are in `activity_nagger.log` in
the data directory (`%LOCALAPPDATA%\activity_nagger\` or `AN_DATA_DIR`).

## Tray menu (Windows)

1. `npm run tauri dev` (or start the installed app).
2. Right-click the tray icon and use each item once.

Pass: the log has `tray menu: <id>` for each item, and

| Item | Effect |
|---|---|
| Open dashboard | dashboard window comes up |
| Start break now | break overlay with a countdown on every monitor |
| Pause tracking for 30 min | item text turns into "Resume tracking"; dashboard shows paused |
| Reading mode | item gets a check mark; dashboard's Reading mode button is on |
| Pair browser extension… | pairing window with a 6-digit code |
| Quit | app exits; the log ends with `shutting down` |

Left-clicking the icon opens the dashboard; the tooltip names the app in front.

## Nag during an exclusive-fullscreen game (Windows)

1. In the Rules tab, add a quick test rule and save:

   ```toml
   [[rules]]
   id = "fullscreen-test"
   trigger = { kind = "interval", every = "1m", break_for = "10s" }
   escalation = [ { after = "0s", level = "overlay" } ]
   ```

2. Start a game in exclusive fullscreen (not borderless) and play for a
   minute.

Pass: a beep, the log line `overlay deferred: exclusive fullscreen`, the tray
tooltip names the rule, no overlay over the game. Alt-Tab out (or switch the
game to windowed): the overlay appears within a second. Remove the test rule
afterwards.

## Linux desktop

Automated on the Xubuntu VM (2026-11-03_230747_1793743667):
`tools/linux_checks.sh` (X11 under Xvfb + xfwm4, Wayland under headless
sway, logind, autostart: 14/14) and `tools/linux_app_check.sh` (the built app
under Xvfb: API, tracking of an xterm, the break overlay: 8/8); the `.deb`
installs, runs and removes. What still needs a real desktop session:

Build on the target machine (WebKitGTK packages as in
`.github/workflows/ci.yml`), then `npm run tauri dev`.

- The tray icon in a panel with a StatusNotifier host (Xfce, KDE): the menu
  works and the tooltip updates.

- X11 session, and a wlroots Wayland compositor (Sway, Hyprland): the
  dashboard header shows the app in front; switching apps changes it within
  a few seconds; leaving the machine alone for `idle_after_s` shows "idle".
- `cargo run -p an-platform --example probe` prints the focused app, idle
  time and Steam state.
- Lock the screen (`loginctl lock-session`): the timeline gets a `locked`
  segment. Suspend and resume: a `suspended` segment.
- A `lock`-level nag locks the session.
- Settings, "Start when you log in": `~/.config/autostart/activity_nagger.desktop`
  appears and disappears with the checkbox.
- GNOME on Wayland: only X clients are tracked and idle is unsupported
  (expected, see docs/DECISIONS.md D17).

## GitHub CI and releases

Push to a GitHub repository: the `CI` workflow's `windows` and `linux` jobs
must pass. Push a tag `v0.1.0`: the `Release` workflow builds a draft
release with the NSIS installer, `.deb`, AppImage and both extension zips.
Signing is covered in docs/RELEASING.md.

## Extension, by hand

`tools/ext_e2e.ps1` covers loading, pairing through the API, event delivery
and heartbeats in Edge and Firefox. By hand: load the unpacked extension,
type the code from Settings into its popup, browse with the browser in front
for a few minutes. Pass: the Web tab lists the sites with plausible times,
and nothing accrues while another app is in front or you are idle.
