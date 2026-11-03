# Backlog and risks

## Risks

- **Win32 process info**: protected processes refuse even
  `PROCESS_QUERY_LIMITED_INFORMATION`; they show as `<elevated>`. Elevation is
  inferred when the token cannot be opened, which also flags anti-cheat
  protected games (seen with a Steam game) as elevated. A `GetProcessImageFileNameW`
  fallback would not help: it needs the same access right.
- **Idle and gamepads**: `GetLastInputInfo` ignores XInput; mitigated by
  `idle_policy = ignore` for games. Later: optional `XInputGetState` poll.
- **Overlays vs exclusive fullscreen**: always-on-top windows cannot cover
  exclusive-fullscreen D3D. `WindowInfo.is_fullscreen` is detected; still to
  do: `SHQueryUserNotificationState`, degrade to sound + tray notice and
  re-raise when the game loses the foreground.
- **DPI**: overlays are positioned in physical pixels per monitor; needs
  checks on mixed-DPI setups.
- **Tray from other desktops**: a process on a hidden desktop can still
  register a tray icon on the user's taskbar; tests use `--no-tray`.

## Deferred

- Steam `appmanifest_*.acf` names, a user-editable `known_games.toml`.
- Rule editor UI beyond the TOML editor; per-rule enable toggle in the
  dashboard.
- Category stats over weeks (charts), CSV/JSON export, DB backup.
- Autostart toggle in Settings (`Platform::set_autostart` exists).
- Sound on nag; per-rule quiet hours.
- Extension store packaging (Chrome Web Store, AMO); signed Firefox build.
- Linux backend (M5), installer signing and CI (M6).
