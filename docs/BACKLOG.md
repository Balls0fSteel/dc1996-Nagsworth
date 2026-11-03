# Backlog and risks

## Risks

- **Win32 process info**: protected processes refuse even
  `PROCESS_QUERY_LIMITED_INFORMATION`; they show as `<elevated>`. Elevation is
  inferred when the token cannot be opened, which also flags anti-cheat
  protected games as elevated (harmless). A `GetProcessImageFileNameW`
  fallback would not help: it needs the same access right.
- **Idle and gamepads**: `GetLastInputInfo` ignores XInput; mitigated by
  `idle_policy = ignore` for games. Later: optional `XInputGetState` poll.
- **Exclusive fullscreen**: overlays wait (beep + tray tooltip) until the
  game leaves exclusive mode; a game that never does is only nagged by sound.
  The deferral path is unit-tested; it has not yet been seen with a real
  exclusive-fullscreen game.
- **Linux**: checked on the Xubuntu VM under Xvfb and headless sway, not yet
  in a real desktop session (panel tray, KDE, GNOME). GNOME Wayland tracks X
  clients only. On Wayland idle is counted from the app's start (the
  compositor reports only "idle since now"), so idle right after a start can
  be under-counted.
- **Tray from other desktops**: a process on a hidden desktop can still
  register a tray icon on the user's taskbar; tests use `--no-tray`.

## Deferred

- Rule editor UI beyond the TOML editor; per-rule enable toggle.
- Week/month charts (data is in `/stats/daily`).
- Sound choice per rule; per-rule quiet hours.
- Flatpak package ids on Linux (cgroup lookup); KDE Wayland window tracking
  (`org_kde_plasma_window_management`).
- Publishing to the extension stores (packages are built; listings are not).
