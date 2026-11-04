//! OS integration behind the [`Platform`] trait.
//!
//! Everything here is polling or documented notification APIs: no global
//! hooks, no DLL injection and no reading other processes' memory.

use std::sync::Arc;
use std::time::Duration;

use ns_core::{ProcessInfo, Sample, SessionEvent, SessionState, Timestamp, WindowInfo};

pub mod mock;

pub mod linux;
#[cfg(windows)]
pub mod windows;

pub use mock::MockPlatform;

#[derive(Debug, thiserror::Error)]
pub enum PlatformError {
    #[error("{0} is not supported on this platform")]
    Unsupported(&'static str),
    #[error("OS error: {0}")]
    Os(String),
}

pub type Result<T> = std::result::Result<T, PlatformError>;

/// Called from a platform thread for every session change.
pub type SessionSink = Box<dyn Fn(SessionEvent) + Send + Sync>;

/// What a platform backend can do, for the UI to grey out missing features.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Caps {
    pub session_events: bool,
    pub lock_workstation: bool,
    pub autostart: bool,
    pub steam: bool,
    /// Controller input counts as input ([`Platform::gamepad_idle_time`]).
    pub gamepad: bool,
}

/// Steam's own view of what it is running, read from its registry keys.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SteamInfo {
    /// Non-zero while a Steam game runs.
    pub running_app_id: u32,
    pub steam_path: Option<String>,
}

pub trait Platform: Send + Sync {
    /// The window that has the keyboard focus, if any.
    fn foreground_window(&self) -> Result<Option<WindowInfo>>;

    /// Time since the last keyboard or mouse input in this session.
    fn idle_time(&self) -> Result<Duration>;

    /// Time since the last controller input. `None` when the backend has no
    /// gamepad support, polling is off or no controller input was seen yet.
    fn gamepad_idle_time(&self) -> Option<Duration> {
        None
    }

    fn process_info(&self, pid: u32) -> Result<ProcessInfo>;

    /// Starts delivering lock/unlock and suspend/resume events to `sink`.
    fn session_events(&self, sink: SessionSink) -> Result<()>;

    /// Current session state as last reported by session events.
    fn session_state(&self) -> SessionState {
        SessionState::Active
    }

    fn autostart(&self) -> Result<bool>;

    fn set_autostart(&self, enabled: bool) -> Result<()>;

    fn lock_workstation(&self) -> Result<()>;

    fn steam_info(&self) -> Option<SteamInfo> {
        None
    }

    /// An exclusive-fullscreen game or a presentation owns the screen, so
    /// always-on-top windows would not be seen.
    fn exclusive_fullscreen(&self) -> bool {
        false
    }

    /// A short attention sound, for nags that cannot be shown.
    fn alert(&self) {}

    /// Turns controller polling on or off (`tracking.gamepad`). Backends
    /// without gamepad support ignore it.
    fn set_gamepad_polling(&self, _on: bool) {}

    fn caps(&self) -> Caps;

    /// One poll: foreground window, idle time and session state. Idle is the
    /// time since the last input on any device: keyboard, mouse or, where
    /// supported, a game controller. Errors are logged and turned into an
    /// empty sample so tracking keeps going.
    fn sample(&self, ts: Timestamp) -> Sample {
        let fg = self.foreground_window().unwrap_or_else(|e| {
            tracing::debug!("foreground_window failed: {e}");
            None
        });
        let mut idle = self.idle_time().unwrap_or_else(|e| {
            tracing::debug!("idle_time failed: {e}");
            Duration::ZERO
        });
        if let Some(pad) = self.gamepad_idle_time() {
            idle = idle.min(pad);
        }
        let idle_ms = idle.as_millis() as u64;
        Sample {
            ts,
            fg,
            idle_ms,
            session: self.session_state(),
        }
    }
}

/// The backend for the current OS.
pub fn create_platform() -> Arc<dyn Platform> {
    #[cfg(windows)]
    {
        Arc::new(windows::WindowsPlatform::new())
    }
    #[cfg(target_os = "linux")]
    {
        Arc::new(linux::LinuxPlatform::new())
    }
    #[cfg(not(any(windows, target_os = "linux")))]
    {
        tracing::warn!("no platform backend for this OS; nothing is tracked");
        Arc::new(MockPlatform::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixed {
        idle: Duration,
        pad: Option<Duration>,
    }

    impl Platform for Fixed {
        fn foreground_window(&self) -> Result<Option<WindowInfo>> {
            Ok(None)
        }
        fn idle_time(&self) -> Result<Duration> {
            Ok(self.idle)
        }
        fn gamepad_idle_time(&self) -> Option<Duration> {
            self.pad
        }
        fn process_info(&self, pid: u32) -> Result<ProcessInfo> {
            Ok(ProcessInfo::unknown(pid))
        }
        fn session_events(&self, _sink: SessionSink) -> Result<()> {
            Ok(())
        }
        fn autostart(&self) -> Result<bool> {
            Ok(false)
        }
        fn set_autostart(&self, _enabled: bool) -> Result<()> {
            Ok(())
        }
        fn lock_workstation(&self) -> Result<()> {
            Ok(())
        }
        fn caps(&self) -> Caps {
            Caps::default()
        }
    }

    #[test]
    fn sample_folds_in_gamepad_idle() {
        let s = |idle, pad| Fixed { idle, pad }.sample(0).idle_ms;
        let secs = Duration::from_secs;
        assert_eq!(s(secs(300), None), 300_000);
        assert_eq!(s(secs(300), Some(secs(2))), 2_000);
        assert_eq!(s(secs(1), Some(secs(60))), 1_000);
    }
}
