//! Domain types shared by every Nagsworth crate: tracking samples and
//! segments, the event bus, the clock abstraction and the config file.
//!
//! This crate does no I/O except reading and writing the config file.

pub mod clock;
pub mod config;
pub mod day;
pub mod event;
pub mod types;

pub use clock::{Clock, MockClock, SystemClock};
pub use config::Config;
pub use day::{LocalDay, TimeOfDay};
pub use event::{
    Activity, AppRef, ControlCommand, DismissReason, Event, EventBus, NagEvent, NagLevel,
    SessionEvent, TrackerEvent, WebEvent,
};
pub use types::{
    CategoryKind, ProcessInfo, Sample, Segment, SegmentKind, SessionState, Timestamp, WindowInfo,
};
