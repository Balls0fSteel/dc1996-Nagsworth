use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::day::LocalDay;

/// Milliseconds since the Unix epoch, UTC.
pub type Timestamp = i64;

/// A process that owns a foreground window.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessInfo {
    pub pid: u32,
    /// File name of the executable, e.g. `firefox.exe`. `<elevated>` or
    /// `<unknown>` when the process cannot be queried.
    pub exe_name: String,
    pub exe_path: Option<String>,
    pub is_elevated: bool,
    /// Package family name for packaged (UWP/MSIX) apps.
    pub package_id: Option<String>,
}

impl ProcessInfo {
    pub fn unknown(pid: u32) -> Self {
        Self {
            pid,
            exe_name: "<unknown>".into(),
            exe_path: None,
            is_elevated: false,
            package_id: None,
        }
    }
}

/// The foreground window at the time of a sample.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowInfo {
    pub hwnd: u64,
    pub title: String,
    pub process: ProcessInfo,
    /// The window covers its whole monitor (fullscreen game or video).
    pub is_fullscreen: bool,
    pub monitor: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionState {
    #[default]
    Active,
    Locked,
    Suspended,
}

/// One poll of the platform. Samples are never stored; the segmenter turns
/// them into segments.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sample {
    pub ts: Timestamp,
    pub fg: Option<WindowInfo>,
    /// Time since the last keyboard or mouse input.
    pub idle_ms: u64,
    pub session: SessionState,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SegmentKind {
    Active,
    Idle,
    Locked,
    Suspended,
    Paused,
}

impl SegmentKind {
    pub const ALL: [SegmentKind; 5] = [
        SegmentKind::Active,
        SegmentKind::Idle,
        SegmentKind::Locked,
        SegmentKind::Suspended,
        SegmentKind::Paused,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            SegmentKind::Active => "active",
            SegmentKind::Idle => "idle",
            SegmentKind::Locked => "locked",
            SegmentKind::Suspended => "suspended",
            SegmentKind::Paused => "paused",
        }
    }

    /// Time of this kind counts as rest for break rules.
    pub fn is_rest(self) -> bool {
        !matches!(self, SegmentKind::Active)
    }
}

impl fmt::Display for SegmentKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for SegmentKind {
    type Err = UnknownVariant;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|k| k.as_str() == s)
            .ok_or_else(|| UnknownVariant(s.to_string()))
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CategoryKind {
    Work,
    Game,
    Web,
    #[default]
    Other,
}

impl CategoryKind {
    pub const ALL: [CategoryKind; 4] = [
        CategoryKind::Work,
        CategoryKind::Game,
        CategoryKind::Web,
        CategoryKind::Other,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            CategoryKind::Work => "Work",
            CategoryKind::Game => "Game",
            CategoryKind::Web => "Web",
            CategoryKind::Other => "Other",
        }
    }
}

impl fmt::Display for CategoryKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for CategoryKind {
    type Err = UnknownVariant;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|k| k.as_str().eq_ignore_ascii_case(s))
            .ok_or_else(|| UnknownVariant(s.to_string()))
    }
}

#[derive(Debug, thiserror::Error)]
#[error("unknown variant `{0}`")]
pub struct UnknownVariant(pub String);

/// A stretch of time with one (app, title, kind). Idle, locked, suspended and
/// paused segments have no app.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Segment {
    pub id: i64,
    pub app_id: Option<i64>,
    pub title_id: Option<i64>,
    pub kind: SegmentKind,
    pub start: Timestamp,
    pub end: Timestamp,
    pub local_day: LocalDay,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds_round_trip_through_strings() {
        for k in SegmentKind::ALL {
            assert_eq!(k.as_str().parse::<SegmentKind>().unwrap(), k);
        }
        for c in CategoryKind::ALL {
            assert_eq!(c.as_str().parse::<CategoryKind>().unwrap(), c);
        }
        assert_eq!("game".parse::<CategoryKind>().unwrap(), CategoryKind::Game);
        assert!("nope".parse::<SegmentKind>().is_err());
    }
}
