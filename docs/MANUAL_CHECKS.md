# Manual checks

What automated runs on the development PC cannot cover. Each check says how
to run it and what passing looks like. Logs are in `nagsworth.log` in
the data directory (`%LOCALAPPDATA%\nagsworth\` or `NAGSWORTH_DATA_DIR`).

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
| Check for updates… | dashboard opens on Settings; the Updates card says "Checking…" then the answer (the item reads "Update to <version>…" once one is known) |
| Quit | app exits; the log ends with `shutting down` |

Left-clicking the icon opens the dashboard; the tooltip names the app in front.

## Nag during an exclusive-fullscreen game (Windows)

1. In the Rules tab (Config file view), add a quick test rule and save:

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

## Hands-on gaming time (Windows, needs a controller)

1. Plug in an XInput controller (Xbox pad, or any pad through Steam Input)
   and start a game, or any app you set to the Game category on the Apps
   page.
2. Play with the controller only for two minutes; then put it down for two
   minutes with the game still in front.

Pass: the status block in the sidebar reads "Playing" while you use the pad
and "In front" once you stop; `/v1/state` shows `activity.hands_on` flip the
same way and `idle_ms` stays small while you only use the pad. On Today the
Game tile shows about 2 min hands-on "of 4 min in front", and the
`gaming-daily` rule's progress grows by about 2 min, not 4. Set
`hands_on_games = false` on Settings: the rule now counts all 4 minutes
(the tile keeps both numbers).

## Update check (Windows)

Without a published release the check answers "newest version". To see
the whole path, serve a fake listing and a fake release from a folder:

1. Make `fake\releases.json` with `[{"tag_name": "v9.9.9", "prerelease": false}]`,
   copy the built setup there as `Nagsworth_9.9.9_x64-setup.exe` and write
   `SHA256SUMS` beside it (`sha256sum`'s `<hex>  <name>` line; in
   PowerShell: `"$((Get-FileHash $f).Hash.ToLower())  Nagsworth_9.9.9_x64-setup.exe"`).
2. `python -m http.server 8765 --bind 127.0.0.1` in that folder.
3. Install Nagsworth with the setup, then start the installed app with
   `NAGSWORTH_UPDATE_URL=http://127.0.0.1:8765/releases.json` and
   `NAGSWORTH_UPDATE_FILES=http://127.0.0.1:8765` set.

Pass: within a minute the dashboard shows the banner "Nagsworth 9.9.9 is
available" and the log `update check: 9.9.9 is available`; "Later" hides
the banner, and a restart does not bring it back (`update.seen`). Settings
> Updates: "Download update" shows progress and ends in "Install and
restart"; that closes the app, the setup runs (the log ends with `setup for
9.9.9 started, exiting`), and the app is back in the tray within about ten
seconds, still at the real version (the fake release is the same build).
Edit `SHA256SUMS` to a wrong hash: the download fails with "checksum does
not match" and leaves no file in `%TEMP%\Nagsworth-update`. An unpacked
copy (`target\release\nagsworth.exe`) offers "Open the release page"
instead.

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
  dashboard's sidebar status shows the app in front; switching apps changes it within
  a few seconds; leaving the machine alone for `idle_after_s` shows "idle".
- `cargo run -p ns-platform --example probe` prints the focused app, idle
  time and Steam state.
- Lock the screen (`loginctl lock-session`): the timeline gets a `locked`
  segment. Suspend and resume: a `suspended` segment.
- A `lock`-level nag locks the session.
- Settings, "Start when you log in": `~/.config/autostart/nagsworth.desktop`
  appears and disappears with the checkbox.
- GNOME on Wayland: only X clients are tracked and idle is unsupported
  (expected, see docs/DECISIONS.md D17).

## GitHub CI and releases

Push to a GitHub repository: the `CI` workflow's `windows` and `linux` jobs
must pass. Push a tag `v0.1.0`: the `Release` workflow builds a draft
release with the NSIS installer (installed, updated in place and
uninstalled on the runner first), `.deb`, AppImage, both extension zips and
`SHA256SUMS`. Signing is covered in docs/RELEASING.md. After publishing the
release, an installed older version must offer it within a minute of a
start.

## Extension, by hand

`tools/ext_e2e.ps1` covers loading, pairing through the API, event delivery
and heartbeats in Edge and Firefox, and in Edge opens the popup as a tab and
checks it shows the paired status and a host (screenshot in
`%TEMP%an-ext-e2epopup-edge.png`). It cannot cover what needs a browser
in front, the real toolbar or a person clicking. By hand
(added 2026-11-04_140801_1793797681):

1. **Pairing and counting.** Load the unpacked extension, open its settings
   (right-click the icon > Options/Manage), type the code from the
   dashboard's Settings, browse with the browser in front for a few
   minutes. Pass: the Web tab lists the sites with plausible times, and
   nothing accrues while another app is in front or you are idle.
2. **Popup contents.** Click the toolbar icon. Pass: "Paired as <browser> on
   port 47821, last sent … ago"; Now shows the current site with
   "Counting" (or "Idle" after a few idle minutes); Today lists up to five
   hosts with the same times as the dashboard's Web page; Nags shows the
   enabled rules with progress. With the app closed the popup says "not
   running" within about 2 s.
3. **Popup controls.** With a rule due (a 1-minute interval rule), Snooze
   and Skip appear and work like the toast's buttons; "Start break now"
   shows the overlay; "Pause tracking 30 min" turns into "Resume tracking"
   and the dashboard shows paused.
4. **Badge.** Turn on "Show today's web time on the toolbar icon". Pass: the
   icon shows e.g. "12m" or "1h" within a minute and when the popup opens;
   turning it off clears it.
5. **Private-window hint.** Tick "Also report incognito / private tabs"
   without allowing the extension in private windows. Pass: the settings
   page says where to allow it (`about:addons` in Firefox, the extensions
   page in Chromium). Open a private window and focus it: the dashboard's
   current web visit ends (`focus_lost`) instead of the last normal tab
   continuing.
6. **Forget everything.** Click it twice. Pass: the page says "Not paired
   yet", the popup asks to pair, and the dashboard no longer gets events
   from this browser.
7. **Firefox popup.** Firefox has no DevTools hook in the harness: repeat 2
   to 4 in Firefox, also in its dark theme.
8. **Worker restart (Chrome).** Close the app, wait for a failed flush,
   then stop the service worker on `chrome://extensions` (Inspect views >
   service worker > stop). Pass: `nagsworth.log` shows no burst of requests
   when the app restarts; queued events arrive within about 5 min.
