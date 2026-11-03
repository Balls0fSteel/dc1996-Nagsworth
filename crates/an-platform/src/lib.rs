//! OS integration behind the [`Platform`] trait.
//!
//! Everything here is polling or documented notification APIs: no global
//! hooks, no DLL injection and no reading other processes' memory.

use std::sync::Arc;
use std::time::Duration;

use an_core::{ProcessInfo, Sample, SessionEvent, SessionState, Timestamp, WindowInfo};

pub mod mock;

#[cfg(not(windows))]
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

    fn caps(&self) -> Caps;

    /// One poll: foreground window, idle time and session state. Errors are
    /// logged and turned into an empty sample so tracking keeps going.
    fn sample(&self, ts: Timestamp) -> Sample {
        let fg = self.foreground_window().unwrap_or_else(|e| {
            tracing::debug!("foreground_window failed: {e}");
            None
        });
        let idle_ms = self
            .idle_time()
            .map(|d| d.as_millis() as u64)
            .unwrap_or_default();
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
    #[cfg(not(windows))]
    {
        Arc::new(linux::LinuxPlatform::new())
    }
}
