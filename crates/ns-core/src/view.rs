//! What the service shows its clients: the open nags and which nag windows
//! they need, and the update state. The API serves these and sends them on
//! the event stream; the GUI only renders them (docs/HEADLESS.md).

use serde::{Deserialize, Serialize};

use crate::event::{NagLevel, Routine};
use crate::types::Timestamp;

/// One open nag as the toast and the overlay render it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NagView {
    pub rule_id: String,
    pub rule_name: String,
    pub message: String,
    pub level: NagLevel,
    pub since: Timestamp,
    pub break_for_ms: Option<u64>,
    pub snoozes_left: u32,
    /// Set while a started break runs.
    pub break_until: Option<Timestamp>,
    /// The exercises to suggest (docs/EXERCISES.md).
    pub routine: Option<Routine>,
}

/// The open nags and the windows a GUI should show for them.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NagsView {
    pub nags: Vec<NagView>,
    /// Show the toast window.
    pub toast: bool,
    /// Show the overlay on every monitor.
    pub overlay: bool,
    /// An overlay waits for an exclusive-fullscreen app to let go; the
    /// service beeped, and the tray names the waiting nags.
    pub deferred: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AvailableView {
    pub version: String,
    /// The release's page, with its notes and files.
    pub url: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DownloadView {
    pub got: u64,
    pub total: Option<u64>,
}

/// The update state the dashboard's banner, Settings card and tray show.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateStatus {
    /// The running version.
    pub current: String,
    /// Whether this build checks at all (`NAGSWORTH_NO_UPDATE_CHECK`).
    pub enabled: bool,
    pub auto_check: bool,
    pub beta: bool,
    /// Installed by the setup: it can install an update itself.
    pub can_install: bool,
    pub checking: bool,
    pub available: Option<AvailableView>,
    /// The version the banner shows, until "Later".
    pub banner: Option<String>,
    pub download: Option<DownloadView>,
    /// A setup is downloaded and checked: "Install and restart" works.
    pub ready: Option<String>,
    pub error: Option<String>,
    /// When the last check answered, unix ms.
    pub last_check: Option<i64>,
}

/// Requests from the service to the program hosting it or showing it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AppEvent {
    /// The update's setup is running and waits for this process: exit now.
    ExitForUpdate,
}
