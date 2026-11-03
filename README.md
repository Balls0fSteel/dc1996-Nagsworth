# activity_nagger

An open source activity tracker and break nagger for your own PC (Windows
first, Linux planned). Working name; a real one is still to be chosen.

- **Tracks** which app is in front and for how long (with idle, lock and sleep
  detection), how much of it is gaming (Steam, launcher folders, known games,
  your own categories), and which websites you look at (with the browser
  extension).
- **Nags** like a modern Workrave: interval breaks and micro-breaks, reading
  mode, daily limits per category ("2h of gaming"), continuous-use limits and
  schedules ("no gaming 23:00-06:00"), escalating from a toast to a
  fullscreen overlay on every monitor to (optionally) locking the workstation.

Everything stays on your machine: one SQLite file under
`%LOCALAPPDATA%\activity_nagger\`, and a local API bound to `127.0.0.1`.
No global hooks, no injection, no reading other processes' memory.

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

`npm run tauri build` makes an installer (NSIS). The app starts in the tray;
left-click the icon for the dashboard. Closing the dashboard keeps it running;
use the tray's Quit.

Command-line flags: `--hidden` / `--autostart` start without showing the
dashboard; `--no-tray` skips the tray icon (tests). `AN_DATA_DIR` overrides the
data directory; `RUST_LOG` the log level (logs go to `activity_nagger.log` in
the data directory).

### Browser extension

```bash
npm run ext:build
```

Load `extension/dist/chrome` as an unpacked extension (Chrome, Edge, Brave)
or `extension/dist/firefox/manifest.json` as a temporary add-on (Firefox).
Then pair it: dashboard, Settings, "Show pairing code", and enter the code in
the extension's popup.

## Configuration

The config file is `%LOCALAPPDATA%\activity_nagger\config.toml`, written from
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

`cargo run -p an-platform --example probe` prints what the platform layer sees
(foreground app, idle time, Steam state) once per second.

## Layout

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for how the pieces fit and
[docs/DECISIONS.md](docs/DECISIONS.md) for why. Current work is in
[PLAN.md](PLAN.md), history in [CHANGELOG.md](CHANGELOG.md).

## License

MIT OR Apache-2.0, at your option.
