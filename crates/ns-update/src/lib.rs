//! The update check and, on Windows, Nagsworth updating itself (docs/DECISIONS.md D22, D23),
//! after PaTorrent's updater.
//!
//! * [`fetch`] asks GitHub which releases exist (the releases API, [`RELEASES_API_URL`]) and
//!   makes a [`Manifest`] of them: the newest release and the newest pre-release.
//! * [`Checker`] decides when: [`FIRST_CHECK`] after the start, then every [`INTERVAL`], while
//!   "Check for updates automatically" is on; "Check now" asks at once. The app runs [`fetch`]
//!   on a worker thread and hands the result to [`Checker::finish`].
//! * A newer version is shown in the dashboard (a banner, and the Settings card) and in the tray
//!   menu. The banner's "Later" is remembered per version as `update.seen` in the settings
//!   table ([`Checker::dismissed`]), so a later start shows the banner for a newer version only.
//! * On Windows, an installed Nagsworth (the setup's, [`installed`]) updates itself:
//!   [`download_setup`] fetches the release's setup and its `SHA256SUMS` from GitHub and checks
//!   the file against the list, and the app starts the setup silently ([`setup_args`]:
//!   `/S /UPDATE /R /WAITPID=<pid>`, so it waits for the exiting Nagsworth, installs over the
//!   same folder and starts the new version hidden) and exits. A portable copy, and Linux, get
//!   the release's page instead.
//! * A pre-release is offered when "Include pre-releases" is on, or when the running version
//!   is a pre-release itself.
//! * A build with `NAGSWORTH_NO_UPDATE_CHECK` set at compile time has no check at all
//!   ([`AVAILABLE`]), for packages that a package manager updates.
//! * For a hand test, `NAGSWORTH_UPDATE_URL` at run time names another releases listing (a
//!   local `http://127.0.0.1:8000/releases.json`, say) and `NAGSWORTH_UPDATE_FILES` a folder
//!   URL holding a release's setup and `SHA256SUMS`; plain `http://` is allowed only then.

mod checker;
mod fetch;
mod install;
mod manifest;
mod version;

pub use checker::{Answer, Checker, FIRST_CHECK, Finished, INTERVAL, Kind};
pub use fetch::{Setup, download_setup, expected_sha256, fetch, sha256_of};
pub use install::{Installed, download_dir, installed, run_setup, same_folder, setup_args};
pub use manifest::Manifest;
pub use version::{RELEASES_API_URL, RELEASES_URL, REPO_URL, Version};

/// Whether this build checks for updates at all.
pub const AVAILABLE: bool = option_env!("NAGSWORTH_NO_UPDATE_CHECK").is_none();

/// Whether this build can install an update itself (Windows, when [`installed`] says so).
pub const CAN_SELF_UPDATE: bool = cfg!(windows);
