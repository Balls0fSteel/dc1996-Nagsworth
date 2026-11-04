//! Local days and times of day.
//!
//! A [`LocalDay`] is the calendar date of a timestamp in local time, shifted
//! by the configured day start: with `day_start = 04:00`, 02:30 on Tuesday
//! still belongs to Monday.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use time::{Date, Month, OffsetDateTime, UtcOffset};

use crate::types::Timestamp;

/// A time of day with minute precision, written `HH:MM`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TimeOfDay {
    minutes: u16,
}

impl TimeOfDay {
    pub const MIDNIGHT: TimeOfDay = TimeOfDay { minutes: 0 };

    pub fn new(hour: u8, minute: u8) -> Option<Self> {
        (hour < 24 && minute < 60).then_some(Self {
            minutes: hour as u16 * 60 + minute as u16,
        })
    }

    /// Minutes since midnight.
    pub fn minutes(self) -> u16 {
        self.minutes
    }

    /// The local time of day of `ts`.
    pub fn of(ts: Timestamp, offset_s: i32) -> Self {
        let local_ms = ts + offset_s as i64 * 1000;
        let minutes = local_ms.div_euclid(60_000).rem_euclid(24 * 60) as u16;
        Self { minutes }
    }
}

impl fmt::Display for TimeOfDay {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:02}:{:02}", self.minutes / 60, self.minutes % 60)
    }
}

#[derive(Debug, thiserror::Error)]
#[error("invalid time of day `{0}`, expected HH:MM")]
pub struct BadTimeOfDay(String);

impl FromStr for TimeOfDay {
    type Err = BadTimeOfDay;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let bad = || BadTimeOfDay(s.to_string());
        let (h, m) = s.trim().split_once(':').ok_or_else(bad)?;
        let h: u8 = h.parse().map_err(|_| bad())?;
        let m: u8 = m.parse().map_err(|_| bad())?;
        // 24:00 is accepted as the end of a schedule window.
        if h == 24 && m == 0 {
            return Ok(Self { minutes: 24 * 60 });
        }
        Self::new(h, m).ok_or_else(bad)
    }
}

impl Serialize for TimeOfDay {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for TimeOfDay {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

/// The local calendar day a timestamp is accounted to, written `YYYY-MM-DD`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LocalDay(pub Date);

impl LocalDay {
    pub fn from_ts(ts: Timestamp, offset_s: i32, day_start: TimeOfDay) -> Self {
        let shifted = ts + (offset_s as i64 - day_start.minutes() as i64 * 60) * 1000;
        let days = shifted.div_euclid(86_400_000);
        let epoch = Date::from_calendar_date(1970, Month::January, 1).expect("valid date");
        LocalDay(epoch + time::Duration::days(days))
    }

    /// The timestamp at which this local day starts.
    pub fn start_ts(self, offset_s: i32, day_start: TimeOfDay) -> Timestamp {
        let epoch = Date::from_calendar_date(1970, Month::January, 1).expect("valid date");
        let days = (self.0 - epoch).whole_days();
        days * 86_400_000 + (day_start.minutes() as i64 * 60 - offset_s as i64) * 1000
    }

    pub fn next(self) -> Self {
        LocalDay(self.0.next_day().unwrap_or(self.0))
    }

    pub fn prev(self) -> Self {
        LocalDay(self.0.previous_day().unwrap_or(self.0))
    }
}

impl fmt::Display for LocalDay {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:04}-{:02}-{:02}",
            self.0.year(),
            self.0.month() as u8,
            self.0.day()
        )
    }
}

#[derive(Debug, thiserror::Error)]
#[error("invalid day `{0}`, expected YYYY-MM-DD")]
pub struct BadLocalDay(String);

impl FromStr for LocalDay {
    type Err = BadLocalDay;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let bad = || BadLocalDay(s.to_string());
        let mut parts = s.trim().splitn(3, '-');
        let y: i32 = parts.next().and_then(|p| p.parse().ok()).ok_or_else(bad)?;
        let m: u8 = parts.next().and_then(|p| p.parse().ok()).ok_or_else(bad)?;
        let d: u8 = parts.next().and_then(|p| p.parse().ok()).ok_or_else(bad)?;
        let month = Month::try_from(m).map_err(|_| bad())?;
        Date::from_calendar_date(y, month, d)
            .map(LocalDay)
            .map_err(|_| bad())
    }
}

impl Serialize for LocalDay {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for LocalDay {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

/// The local UTC offset at `ts` in seconds, falling back to UTC when the OS
/// cannot tell (some Unix setups refuse in multi-threaded processes).
pub fn system_offset_s(ts: Timestamp) -> i32 {
    OffsetDateTime::from_unix_timestamp_nanos(ts as i128 * 1_000_000)
        .ok()
        .and_then(|t| UtcOffset::local_offset_at(t).ok())
        .map(|o| o.whole_seconds())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DAY_MS: i64 = 86_400_000;

    #[test]
    fn time_of_day_parses_and_formats() {
        let t: TimeOfDay = "04:30".parse().unwrap();
        assert_eq!(t.minutes(), 270);
        assert_eq!(t.to_string(), "04:30");
        assert_eq!("24:00".parse::<TimeOfDay>().unwrap().minutes(), 1440);
        assert!("25:00".parse::<TimeOfDay>().is_err());
        assert!("4".parse::<TimeOfDay>().is_err());
    }

    #[test]
    fn local_day_respects_day_start_and_offset() {
        let start: TimeOfDay = "04:00".parse().unwrap();
        // 2026-11-02 02:30 UTC+1 = 2026-11-02 01:30 UTC: still Nov 1.
        let base = LocalDay::from_ts(0, 0, TimeOfDay::MIDNIGHT);
        assert_eq!(base.to_string(), "1970-01-01");
        let nov2: LocalDay = "2026-11-02".parse().unwrap();
        let midnight_utc = nov2.start_ts(0, TimeOfDay::MIDNIGHT);
        let ts = midnight_utc + 90 * 60_000; // 01:30 UTC
        assert_eq!(LocalDay::from_ts(ts, 3600, start).to_string(), "2026-11-01");
        // 05:00 local is the new day.
        let ts = midnight_utc + 4 * 3_600_000;
        assert_eq!(LocalDay::from_ts(ts, 3600, start), nov2);
        assert_eq!(nov2.start_ts(3600, start), midnight_utc + 3 * 3_600_000);
        assert_eq!(
            nov2.next().start_ts(0, start) - nov2.start_ts(0, start),
            DAY_MS
        );
        assert_eq!(nov2.prev().to_string(), "2026-11-01");
    }

    #[test]
    fn time_of_day_of_timestamp() {
        let t = TimeOfDay::of(23 * 3_600_000 + 15 * 60_000, 3600);
        assert_eq!(t.to_string(), "00:15");
    }
}
