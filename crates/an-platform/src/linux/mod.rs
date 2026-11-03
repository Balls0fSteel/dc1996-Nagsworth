//! Linux backend.
//!
//! - Foreground window: wlr-foreign-toplevel on wlroots Wayland compositors,
//!   otherwise EWMH on X11 / XWayland (`_NET_ACTIVE_WINDOW`, `_NET_WM_PID`).
//! - Idle: `ext-idle-notify-v1` on Wayland, MIT-SCREEN-SAVER on X11.
//! - Session lock/unlock and sleep: systemd-logind signals; lock via
//!   `Session.Lock`.
//! - Autostart: an XDG autostart `.desktop` file.
//! - Steam: `~/.steam/registry.vdf`.
//!
//! GNOME on Wayland exposes neither the focused window nor idle time to
//! clients; there only X clients (via XWayland) are tracked.

pub mod parse;

#[cfg(target_os = "linux")]
mod logind;
#[cfg(target_os = "linux")]
mod wayland;
#[cfg(target_os = "linux")]
mod x11;

#[cfg(target_os = "linux")]
pub use platform::LinuxPlatform;

#[cfg(target_os = "linux")]
mod platform {
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU8, Ordering};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use an_core::{ProcessInfo, SessionState, WindowInfo};

    use super::wayland::Shared;
    use super::x11::X11;
    use super::{logind, parse};
    use crate::{Caps, Platform, PlatformError, Result, SessionSink, SteamInfo};

    pub struct LinuxPlatform {
        x11: Option<X11>,
        wayland: Option<Arc<Mutex<Shared>>>,
        session: Arc<AtomicU8>,
    }

    impl Default for LinuxPlatform {
        fn default() -> Self {
            Self::new()
        }
    }

    fn home() -> Option<PathBuf> {
        std::env::var_os("HOME").map(PathBuf::from)
    }

    fn autostart_file() -> Option<PathBuf> {
        let base = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| home().map(|h| h.join(".config")))?;
        Some(base.join("autostart").join("activity_nagger.desktop"))
    }

    impl LinuxPlatform {
        pub fn new() -> Self {
            let wayland = std::env::var_os("WAYLAND_DISPLAY").and_then(|_| super::wayland::spawn());
            let x11 = std::env::var_os("DISPLAY").and_then(|_| X11::connect());
            if wayland.is_none() && x11.is_none() {
                tracing::warn!("no X11 or Wayland display; foreground tracking unavailable");
            }
            Self {
                x11,
                wayland,
                session: Arc::new(AtomicU8::new(0)),
            }
        }

        fn wayland_toplevels(&self) -> bool {
            self.wayland
                .as_ref()
                .is_some_and(|w| w.lock().unwrap().has_toplevels)
        }
    }

    impl Platform for LinuxPlatform {
        fn foreground_window(&self) -> Result<Option<WindowInfo>> {
            if self.wayland_toplevels() {
                let focused = self
                    .wayland
                    .as_ref()
                    .and_then(|w| w.lock().unwrap().focused());
                // Wayland does not give pids; the app id stands in for the exe.
                return Ok(focused.map(|t| WindowInfo {
                    hwnd: 0,
                    title: t.title,
                    process: ProcessInfo {
                        pid: 0,
                        exe_name: t.app_id,
                        exe_path: None,
                        is_elevated: false,
                        package_id: None,
                    },
                    is_fullscreen: t.fullscreen,
                    monitor: 0,
                }));
            }
            let Some(x11) = &self.x11 else {
                return Ok(None);
            };
            let Some(active) = x11.active_window() else {
                return Ok(None);
            };
            let process = match active.pid {
                Some(pid) => self.process_info(pid)?,
                None => ProcessInfo::unknown(0),
            };
            Ok(Some(WindowInfo {
                hwnd: active.window as u64,
                title: active.title,
                process,
                is_fullscreen: active.fullscreen,
                monitor: 0,
            }))
        }

        fn idle_time(&self) -> Result<Duration> {
            if let Some(w) = &self.wayland {
                let w = w.lock().unwrap();
                if w.has_idle {
                    return Ok(Duration::from_millis(w.idle_ms()));
                }
                // XWayland's screensaver extension only sees X clients' input.
                return Err(PlatformError::Unsupported("idle_time on this compositor"));
            }
            self.x11
                .as_ref()
                .and_then(|x| x.idle_ms())
                .map(Duration::from_millis)
                .ok_or(PlatformError::Unsupported("idle_time"))
        }

        fn process_info(&self, pid: u32) -> Result<ProcessInfo> {
            let exe_path = std::fs::read_link(format!("/proc/{pid}/exe")).ok();
            let exe_name = match &exe_path {
                Some(p) => p
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                // Other users' processes hide their exe link; comm is readable.
                None => std::fs::read_to_string(format!("/proc/{pid}/comm"))
                    .map(|s| s.trim().to_string())
                    .unwrap_or_else(|_| "<unknown>".into()),
            };
            let is_elevated = std::fs::read_to_string(format!("/proc/{pid}/status"))
                .ok()
                .and_then(|s| parse::effective_uid(&s))
                == Some(0);
            Ok(ProcessInfo {
                pid,
                exe_name,
                exe_path: exe_path.map(|p| p.to_string_lossy().into_owned()),
                is_elevated,
                // Flatpak apps carry their id in the cgroup; not needed yet.
                package_id: None,
            })
        }

        fn session_events(&self, sink: SessionSink) -> Result<()> {
            logind::watch(sink, self.session.clone()).map_err(PlatformError::Os)
        }

        fn session_state(&self) -> SessionState {
            logind::decode(self.session.load(Ordering::SeqCst))
        }

        fn autostart(&self) -> Result<bool> {
            Ok(autostart_file().is_some_and(|f| f.exists()))
        }

        fn set_autostart(&self, enabled: bool) -> Result<()> {
            let file =
                autostart_file().ok_or(PlatformError::Unsupported("autostart without HOME"))?;
            let io = |e: std::io::Error| PlatformError::Os(e.to_string());
            if enabled {
                let exe = std::env::current_exe().map_err(io)?;
                if let Some(dir) = file.parent() {
                    std::fs::create_dir_all(dir).map_err(io)?;
                }
                std::fs::write(
                    &file,
                    parse::autostart_desktop_entry(&exe.to_string_lossy()),
                )
                .map_err(io)
            } else {
                match std::fs::remove_file(&file) {
                    Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(io(e)),
                    _ => Ok(()),
                }
            }
        }

        fn lock_workstation(&self) -> Result<()> {
            logind::lock_session().map_err(PlatformError::Os)
        }

        fn steam_info(&self) -> Option<SteamInfo> {
            let home = home()?;
            let registry = std::fs::read_to_string(home.join(".steam/registry.vdf")).ok()?;
            let steam_path = [home.join(".steam/steam"), home.join(".local/share/Steam")]
                .into_iter()
                .find_map(|p| std::fs::canonicalize(p).ok())
                .map(|p| p.to_string_lossy().into_owned());
            Some(SteamInfo {
                running_app_id: parse::steam_running_app_id(&registry).unwrap_or(0),
                steam_path,
            })
        }

        fn caps(&self) -> Caps {
            Caps {
                session_events: true,
                lock_workstation: true,
                autostart: true,
                steam: true,
            }
        }
    }
}
