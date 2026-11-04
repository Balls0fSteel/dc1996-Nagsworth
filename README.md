# Nagsworth

Nagsworth is an open source activity tracker and break nagger for your own PC:
Windows, plus a Linux backend for X11 and wlroots Wayland compositors. Think of
a polite butler who keeps a note of your hours and clears his throat when it is
time for a break.

- **Tracks** which app is in front and for how long (with idle, lock and sleep
  detection), how much of it is gaming (Steam, launcher folders, known games,
  your own categories), and which websites you look at (with the browser
  extension).
- **Nags** like a modern Workrave: interval breaks and micro-breaks, reading
  mode, daily limits per category ("2h of gaming"), continuous-use limits and
  schedules ("no gaming 23:00-06:00"), escalating from a toast to a
  fullscreen overlay on every monitor to (optionally) locking the workstation.

Everything stays on your machine: one SQLite file under
`%LOCALAPPDATA%\nagsworth\` (`~/.local/share/nagsworth/` on
Linux), and a local API bound to `127.0.0.1`. The dashboard exports CSV/JSON
and writes database backups (Settings). No global hooks, no injection, no reading other processes' memory.

## Build and run

Needs Rust (stable, 1.90+), Node 20.19+ and, on Windows, WebView2.

```bash
npm install
```

```bash
npm --prefix ui install
```

```bash
npm --prefix extension install
```

```bash
npm run tauri dev
```

On Windows, local helper scripts in the root (kept out of git) wrap these: `1_build.bat` (release, or `debug`),
`2_run.bat`, `3_build_installer.bat`, `4_run_installer.bat` (`/S` for silent)
and `5_dev.bat`.

`npm run tauri build` makes an installer (NSIS on Windows, `.deb` and
AppImage on Linux); see [docs/RELEASING.md](docs/RELEASING.md) for releases,
signing and the extension store packages (`npm run ext:package`).

The app starts in the tray; left-click the icon for the dashboard. Closing
the dashboard keeps it running; use the tray's Quit.

### Updates

Nagsworth asks GitHub for the list of releases 30 s after a start and twice
a day (Settings > Updates to turn that off, to include pre-releases, or to
check now; the tray has "Check for updates…" too). A newer version shows as
a banner and in the tray item. On Windows, a Nagsworth installed by the
setup downloads the new setup, checks it against the release's
`SHA256SUMS` and, on "Install and restart", runs it silently: the app closes
for a few seconds and comes back in the tray; data and settings stay. On
Linux, and for a copy that was not installed by the setup, the button opens
the release page instead.

Command-line flags: `--hidden` / `--autostart` start without showing the
dashboard; `--no-tray` skips the tray icon (tests). `NAGSWORTH_DATA_DIR` overrides the
data directory; `RUST_LOG` the log level (logs go to `nagsworth.log` in
the data directory). `NAGSWORTH_UPDATE_URL` and `NAGSWORTH_UPDATE_FILES`
point the update check at another releases listing and file folder (for
tests); a build with `NAGSWORTH_NO_UPDATE_CHECK` set has no check at all.

### Browser extension

```bash
npm run ext:build
```

Load `extension/dist/chrome` as an unpacked extension (Chrome, Edge, Brave)
or `extension/dist/firefox/manifest.json` as a temporary add-on (Firefox).
Then pair it: dashboard, Settings, "Show pairing code", and enter the code in
the extension's popup.

### Linux

Foreground tracking uses EWMH on X11/XWayland and
`wlr-foreign-toplevel-management` on wlroots compositors (Sway, Hyprland);
idle uses MIT-SCREEN-SAVER or `ext-idle-notify-v1`; lock and sleep come from
systemd-logind. GNOME on Wayland exposes neither the focused window nor idle
time, so there only X clients are tracked. Building needs the WebKitGTK
packages listed in `.github/workflows/ci.yml`.

## Configuration

The config file is `%LOCALAPPDATA%\nagsworth\config.toml`, written from
[config/default.toml](config/default.toml) on first start, and editable in the
dashboard's Rules tab (validated and applied without a restart). Example rule:

```toml
[[rules]]
id = "gaming-daily"
name = "Gaming limit"
scope = { category = "Game" }
trigger = { kind = "daily_total", limit = "2h", renag_every = "15m" }
escalation = [ { after = "0s", level = "toast" }, { after = "10m", level = "overlay" } ]
```

Games: the built-in list of game executables can be extended with
`known_games.toml` in the data directory (`games = ["MyGame.exe"]`, or
`"javaw.exe@minecraft"` to also require a path fragment). Steam games get
their store name as display name.

## Tests

```bash
cargo test --workspace
```

```bash
cargo clippy --workspace --all-targets
```

```bash
npm run ui:check
```

```bash
npm run ext:check
```

`cargo run -p ns-platform --example probe` prints what the platform layer sees
(foreground app, idle time, Steam state) once per second.

## Layout

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for how the pieces fit and
[docs/DECISIONS.md](docs/DECISIONS.md) for why. Current work is in
[PLAN.md](PLAN.md), history in [CHANGELOG.md](CHANGELOG.md).

## License

MIT OR Apache-2.0, at your option.
