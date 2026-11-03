# Agent notes for activity_nagger

Shared preferences live in `L:\dev\AGENTS.md`; this file adds what is
specific to this project. Read [PLAN.md](PLAN.md) and
[CHANGELOG.md](CHANGELOG.md) before larger changes and update both after.

## Crate map

| Crate | Path | Role |
|---|---|---|
| `an-core` | `crates/an-core` | Domain types, `Event` + `EventBus`, `Clock`, `Config`, `LocalDay`. No I/O beyond the config file. |
| `an-platform` | `crates/an-platform` | `Platform` trait; `windows.rs` (Win32), `mock.rs`, `linux.rs` (stub). |
| `an-storage` | `crates/an-storage` | SQLite (`rusqlite`, bundled), migrations, storage actor (`StorageHandle`), stats queries. |
| `an-tracker` | `crates/an-tracker` | `Sampler`, `Segmenter` (pure) + `SegmenterTask`, `Categorizer`, `gaming/`, `WebSegmenter`. |
| `an-rules` | `crates/an-rules` | `RuleSet` (TOML), `Engine` (pure state machine), `EngineTask`. |
| `an-api` | `crates/an-api` | axum router on `127.0.0.1:47821/v1`, bearer auth, pairing, SSE. |
| `an-app` | `src-tauri` | Tauri shell: `runtime.rs` (composition root, no Tauri types), `nag_bridge.rs`, `tray.rs`, `windows.rs`, `commands.rs`. |
| UI | `ui/` | Svelte 5 + TS + Vite, pages `index`, `toast`, `overlay`, `pair`. |
| Extension | `extension/` | MV3, TS, built by `scripts/build.mjs` into `dist/chrome` and `dist/firefox`. |

Dependency direction: `an-core` <- `an-platform`, `an-storage` <- `an-tracker`,
`an-rules` <- `an-api` <- `an-app`.

## Conventions

- No global hooks, DLL injection or reading other processes' memory. Poll
  documented APIs (`GetForegroundWindow`, `GetLastInputInfo`, WTS/power
  notifications, Steam's registry keys). The `lock` nag level calls
  `LockWorkStation`; never try to build an inescapable overlay or use
  `SetForegroundWindow` tricks.
- Timestamps are unix milliseconds UTC (`Timestamp = i64`); days are
  `LocalDay` after `general.day_start`.
- Logic that depends on time takes a `Clock` and is tested with `MockClock`.
  Keep `Segmenter`, `WebSegmenter` and `Engine` pure; I/O lives in their
  `*Task` wrappers.
- Tauri's runtime and the tracking runtime only meet through channels
  (`nag_bridge.rs`, commands reading watch channels).
- Edition 2024, `cargo fmt`, clippy clean (`--all-targets`).

## Build, run, test

```bash
cargo test --workspace
cargo clippy --workspace --all-targets
npm run ui:check
npm run ext:check
npm run ext:build
npm run tauri -- build --debug --no-bundle
```

A plain `cargo build` of `an-app` loads the UI from the Vite dev server
(`npm run ui:dev`); build through the Tauri CLI to embed `ui/dist`.

### Running the app during tests

Start it hidden (see the shared AGENTS.md) with a scratch data directory and
without a tray icon, which would otherwise appear on the user's taskbar:

```powershell
$env:AN_DATA_DIR = "<scratch>\andata"
L:\dev\tools\run_hidden\run_hidden.ps1 -Timeout 20 -Screenshot <scratch>\shot.png -ShotAfter 12 target\debug\activity_nagger.exe --no-tray
```

The API token is in `settings` (`api.token`) in `<data dir>\activity.db`. On the
hidden desktop `GetForegroundWindow` sees no window, so `/state` shows no app;
use `cargo run -p an-platform --example probe` to check foreground detection.
`run_hidden.ps1` does not work inside `Start-Job`; run it in the foreground and
put API calls in the job instead.
