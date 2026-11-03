//! Placeholder Linux backend (milestone M5: X11 via x11rb + XScreenSaver,
//! Wayland via ext-idle-notify-v1, logind over D-Bus).

use std::time::Duration;

use an_core::{ProcessInfo, WindowInfo};

use crate::{Caps, Platform, PlatformError, Result, SessionSink};

#[derive(Debug, Default)]
pub struct LinuxPlatform;

impl LinuxPlatform {
    pub fn new() -> Self {
        Self
    }
}

impl Platform for LinuxPlatform {
    fn foreground_window(&self) -> Result<Option<WindowInfo>> {
        Ok(None)
    }

    fn idle_time(&self) -> Result<Duration> {
        Err(PlatformError::Unsupported("idle_time"))
    }

    fn process_info(&self, pid: u32) -> Result<ProcessInfo> {
        let exe_path = std::fs::read_link(format!("/proc/{pid}/exe")).ok();
        let exe_name = exe_path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "<unknown>".into());
        Ok(ProcessInfo {
            pid,
            exe_name,
            exe_path: exe_path.map(|p| p.to_string_lossy().into_owned()),
            is_elevated: false,
            package_id: None,
        })
    }

    fn session_events(&self, _sink: SessionSink) -> Result<()> {
        Err(PlatformError::Unsupported("session_events"))
    }

    fn autostart(&self) -> Result<bool> {
        Err(PlatformError::Unsupported("autostart"))
    }

    fn set_autostart(&self, _enabled: bool) -> Result<()> {
        Err(PlatformError::Unsupported("autostart"))
    }

    fn lock_workstation(&self) -> Result<()> {
        Err(PlatformError::Unsupported("lock_workstation"))
    }

    fn caps(&self) -> Caps {
        Caps::default()
    }
}
