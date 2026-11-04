//! Events that cross crate boundaries and the broadcast bus that carries them.

use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

use crate::types::{CategoryKind, SegmentKind, Timestamp};

/// An app as the tracker knows it after categorization.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppRef {
    pub id: i64,
    pub exe_name: String,
    pub category: CategoryKind,
    pub is_game: bool,
}

/// What the user is doing right now, published by the segmenter after every
/// sample. The rules engine reads it on each tick.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Activity {
    pub ts: Timestamp,
    /// Raw kind of the latest sample (not debounced).
    pub kind: Option<SegmentKind>,
    /// The foreground app, also while idle.
    pub app: Option<AppRef>,
    pub title: Option<String>,
    pub idle_ms: u64,
    pub is_fullscreen: bool,
    /// The last input on any device (keyboard, mouse, controller) is at most
    /// `tracking.input_window_s` ago and the kind is active or idle.
    pub hands_on: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TrackerEvent {
    SegmentOpened {
        id: i64,
        kind: SegmentKind,
        app: Option<AppRef>,
        title: Option<String>,
        start: Timestamp,
    },
    SegmentTick {
        id: i64,
        end: Timestamp,
    },
    SegmentClosed {
        id: i64,
        end: Timestamp,
    },
    IdleStarted {
        at: Timestamp,
    },
    IdleEnded {
        at: Timestamp,
    },
    ForegroundChanged {
        app: Option<AppRef>,
        title: Option<String>,
        at: Timestamp,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WebEvent {
    VisitStarted {
        id: i64,
        browser: String,
        host: String,
        url: Option<String>,
        title: Option<String>,
        start: Timestamp,
    },
    VisitEnded {
        id: i64,
        end: Timestamp,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SessionEvent {
    Locked { at: Timestamp },
    Unlocked { at: Timestamp },
    Suspending { at: Timestamp },
    Resumed { at: Timestamp },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NagLevel {
    Toast,
    Overlay,
    Lock,
}

/// One exercise of a break routine, with everything the nag windows show.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoutineStep {
    pub id: String,
    pub name: String,
    /// Length of this step in the routine (a walk is stretched to fit).
    pub seconds: u32,
    /// "10 x 3 s", "20 s each side".
    pub reps: String,
    /// One line for the toast.
    pub cue: String,
    /// 1-4 lines for the overlay.
    pub lines: Vec<String>,
}

/// What to do during a break (docs/EXERCISES.md), planned when the break's
/// nag is raised or the break starts.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Routine {
    pub steps: Vec<RoutineStep>,
    /// Why the routine looks the way it does ("2 h 10 min without a real
    /// break: walk first.").
    pub note: Option<String>,
}

impl Routine {
    pub fn total_seconds(&self) -> u32 {
        self.steps.iter().map(|s| s.seconds).sum()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DismissReason {
    /// The user rested long enough (idle, locked, or a started break ended).
    BreakTaken,
    Skipped,
    Snoozed,
    /// The condition no longer holds (left the scope, schedule window ended).
    Resolved,
    /// Rules were reloaded or the rule was disabled.
    Reload,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum NagEvent {
    Raise {
        rule_id: String,
        rule_name: String,
        level: NagLevel,
        message: String,
        at: Timestamp,
        /// Length of the break this nag asks for, if it is a break rule.
        break_for_ms: Option<u64>,
        snoozes_left: u32,
        /// The exercises the break will suggest, if any.
        routine: Option<Routine>,
    },
    Escalate {
        rule_id: String,
        level: NagLevel,
        at: Timestamp,
    },
    BreakStarted {
        rule_id: String,
        until: Timestamp,
        routine: Option<Routine>,
    },
    Dismiss {
        rule_id: String,
        reason: DismissReason,
        at: Timestamp,
    },
}

impl NagEvent {
    pub fn rule_id(&self) -> &str {
        match self {
            NagEvent::Raise { rule_id, .. }
            | NagEvent::Escalate { rule_id, .. }
            | NagEvent::BreakStarted { rule_id, .. }
            | NagEvent::Dismiss { rule_id, .. } => rule_id,
        }
    }
}

/// User commands from the tray, the nag windows or the API.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ControlCommand {
    Snooze {
        rule_id: String,
    },
    SkipBreak {
        rule_id: String,
    },
    /// Start a break now; without a rule id, every break rule takes it.
    StartBreakNow {
        rule_id: Option<String>,
    },
    /// Stop tracking, optionally only for a while.
    PauseTracking {
        for_ms: Option<u64>,
    },
    ResumeTracking,
    SetReadingMode {
        on: bool,
    },
    /// Re-read the config file and rules.
    Reload,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "source", content = "event", rename_all = "snake_case")]
pub enum Event {
    Tracker(TrackerEvent),
    Web(WebEvent),
    Nag(NagEvent),
    Session(SessionEvent),
    Control(ControlCommand),
}

/// Fan-out bus for [`Event`]s. Slow subscribers lose old events rather than
/// blocking publishers.
#[derive(Clone, Debug)]
pub struct EventBus {
    tx: broadcast::Sender<Event>,
}

impl EventBus {
    pub fn new(capacity: usize) -> Self {
        let (tx, _) = broadcast::channel(capacity);
        Self { tx }
    }

    pub fn publish(&self, event: Event) {
        // An error only means nobody is subscribed.
        let _ = self.tx.send(event);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.tx.subscribe()
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new(1024)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn bus_delivers_to_subscribers() {
        let bus = EventBus::default();
        let mut rx = bus.subscribe();
        bus.publish(Event::Control(ControlCommand::Reload));
        assert_eq!(
            rx.recv().await.unwrap(),
            Event::Control(ControlCommand::Reload)
        );
    }

    #[test]
    fn events_serialize_with_tags() {
        let e = Event::Nag(NagEvent::Escalate {
            rule_id: "r".into(),
            level: NagLevel::Overlay,
            at: 5,
        });
        let json = serde_json::to_string(&e).unwrap();
        assert_eq!(
            json,
            r#"{"source":"nag","event":{"type":"escalate","rule_id":"r","level":"overlay","at":5}}"#
        );
        assert!(NagLevel::Toast < NagLevel::Lock);
    }
}
