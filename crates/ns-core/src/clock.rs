use std::sync::atomic::{AtomicI32, AtomicI64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::day::system_offset_s;
use crate::types::Timestamp;

/// Wall-clock time source. Logic that depends on time takes a `Clock` so
/// tests can drive it with [`MockClock`].
pub trait Clock: Send + Sync {
    fn now(&self) -> Timestamp;

    /// Local UTC offset in seconds at `ts`.
    fn offset_s(&self, ts: Timestamp) -> i32;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Timestamp {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as Timestamp)
            .unwrap_or(0)
    }

    fn offset_s(&self, ts: Timestamp) -> i32 {
        system_offset_s(ts)
    }
}

/// A clock that only moves when told to.
#[derive(Debug, Default)]
pub struct MockClock {
    now: AtomicI64,
    offset_s: AtomicI32,
}

impl MockClock {
    pub fn new(now: Timestamp) -> Self {
        Self {
            now: AtomicI64::new(now),
            offset_s: AtomicI32::new(0),
        }
    }

    pub fn with_offset(self, offset_s: i32) -> Self {
        self.offset_s.store(offset_s, Ordering::SeqCst);
        self
    }

    pub fn set(&self, now: Timestamp) {
        self.now.store(now, Ordering::SeqCst);
    }

    pub fn advance(&self, ms: i64) -> Timestamp {
        self.now.fetch_add(ms, Ordering::SeqCst) + ms
    }
}

impl Clock for MockClock {
    fn now(&self) -> Timestamp {
        self.now.load(Ordering::SeqCst)
    }

    fn offset_s(&self, _ts: Timestamp) -> i32 {
        self.offset_s.load(Ordering::SeqCst)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_clock_advances() {
        let c = MockClock::new(1_000).with_offset(3600);
        assert_eq!(c.advance(500), 1_500);
        assert_eq!(c.now(), 1_500);
        assert_eq!(c.offset_s(0), 3600);
        assert!(SystemClock.now() > 1_700_000_000_000);
    }
}
