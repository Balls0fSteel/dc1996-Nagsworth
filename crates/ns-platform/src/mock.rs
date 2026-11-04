//! A scripted platform for tests and for running the app without OS access.

use std::collections::VecDeque;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;

use ns_core::{ProcessInfo, Sample, SessionState, Timestamp, WindowInfo};

use crate::{Caps, Platform, PlatformError, Result, SessionSink};

/// Replays scripted samples. Once the script runs out, the last sample
/// repeats. `sample()` returns the scripted timestamp, not the caller's.
#[derive(Default)]
pub struct MockPlatform {
    script: Mutex<VecDeque<Sample>>,
    last: Mutex<Option<Sample>>,
    autostart: AtomicBool,
    locks: AtomicUsize,
}

impl MockPlatform {
    pub fn scripted(samples: Vec<Sample>) -> Self {
        Self {
            script: Mutex::new(samples.into()),
            ..Default::default()
        }
    }

    /// Number of `lock_workstation` calls so far.
    pub fn lock_count(&self) -> usize {
        self.locks.load(Ordering::SeqCst)
    }

    fn current(&self) -> Option<Sample> {
        self.last.lock().unwrap().clone()
    }
}

/// A window of a fake process, for building scripts.
pub fn window(exe: &str, title: &str) -> WindowInfo {
    WindowInfo {
        hwnd: 1,
        title: title.into(),
        process: ProcessInfo {
            pid: 100,
            exe_name: exe.into(),
            exe_path: Some(format!("C:\\Apps\\{exe}")),
            is_elevated: false,
            package_id: None,
        },
        is_fullscreen: false,
        monitor: 1,
    }
}

/// An active-session sample.
pub fn sample(ts: Timestamp, fg: Option<WindowInfo>, idle_ms: u64) -> Sample {
    Sample {
        ts,
        fg,
        idle_ms,
        session: SessionState::Active,
    }
}

impl Platform for MockPlatform {
    fn foreground_window(&self) -> Result<Option<WindowInfo>> {
        Ok(self.current().and_then(|s| s.fg))
    }

    fn idle_time(&self) -> Result<Duration> {
        Ok(Duration::from_millis(
            self.current().map(|s| s.idle_ms).unwrap_or(0),
        ))
    }

    fn process_info(&self, pid: u32) -> Result<ProcessInfo> {
        self.current()
            .and_then(|s| s.fg)
            .map(|w| w.process)
            .filter(|p| p.pid == pid)
            .ok_or_else(|| PlatformError::Os(format!("no process {pid}")))
    }

    fn session_events(&self, _sink: SessionSink) -> Result<()> {
        Ok(())
    }

    fn session_state(&self) -> SessionState {
        self.current().map(|s| s.session).unwrap_or_default()
    }

    fn autostart(&self) -> Result<bool> {
        Ok(self.autostart.load(Ordering::SeqCst))
    }

    fn set_autostart(&self, enabled: bool) -> Result<()> {
        self.autostart.store(enabled, Ordering::SeqCst);
        Ok(())
    }

    fn lock_workstation(&self) -> Result<()> {
        self.locks.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    fn caps(&self) -> Caps {
        Caps {
            session_events: true,
            lock_workstation: true,
            autostart: true,
            steam: false,
        }
    }

    fn sample(&self, ts: Timestamp) -> Sample {
        let mut script = self.script.lock().unwrap();
        let mut last = self.last.lock().unwrap();
        if let Some(next) = script.pop_front() {
            *last = Some(next);
        }
        last.clone().unwrap_or(Sample {
            ts,
            fg: None,
            idle_ms: 0,
            session: SessionState::Active,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replays_then_repeats_last() {
        let p = MockPlatform::scripted(vec![
            sample(0, Some(window("a.exe", "A")), 0),
            sample(1000, Some(window("b.exe", "B")), 0),
        ]);
        assert_eq!(p.sample(0).fg.unwrap().process.exe_name, "a.exe");
        assert_eq!(p.sample(0).ts, 1000);
        assert_eq!(p.sample(0).ts, 1000);
        assert_eq!(p.foreground_window().unwrap().unwrap().title, "B");
        p.lock_workstation().unwrap();
        assert_eq!(p.lock_count(), 1);
    }
}
