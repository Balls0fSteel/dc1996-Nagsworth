//! When the checks run, and what the dashboard shows afterwards.

use std::time::{Duration, Instant};

use crate::{AVAILABLE, Manifest, Version};

/// The first automatic check after the start.
pub const FIRST_CHECK: Duration = Duration::from_secs(30);
/// Between automatic checks, for a Nagsworth left running.
pub const INTERVAL: Duration = Duration::from_secs(12 * 60 * 60);

/// Who asked for a check.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// The timer: a newer version goes to the dashboard and the log, a failure to the log only.
    Automatic,
    /// "Check now": the answer, a failure too, goes to the Settings card.
    Manual,
}

/// A check's answer: the newer version, `None` when the running one is the newest.
pub type Answer = Result<Option<Version>, String>;

/// What [`Checker::finish`] makes of a check's result.
#[derive(Debug, PartialEq, Eq)]
pub struct Finished {
    /// Who asked.
    pub kind: Kind,
    pub answer: Answer,
}

/// When the checks run, the newer version found, and whether its banner was dismissed. The app
/// calls [`Checker::due`] from a timer and runs the fetch on a thread when it says so (or for
/// [`Checker::start_manual`]), and hands the result to [`Checker::finish`]. The dashboard shows
/// [`Checker::available`] in its Settings card and, until "Later" ([`Checker::dismiss`]), as a
/// banner ([`Checker::banner`]).
#[derive(Debug)]
pub struct Checker {
    /// When the next automatic check is due; `None` while they are off.
    next: Option<Instant>,
    /// A check is on a worker thread; a manual request during an automatic one makes it
    /// manual.
    running: Option<Kind>,
    /// The newer version the last check found.
    available: Option<Version>,
    /// The newest version whose banner was dismissed (`update.seen`).
    dismissed: Option<Version>,
}

impl Checker {
    /// With automatic checks `on` (and [`AVAILABLE`]), the first [`FIRST_CHECK`] from `now`;
    /// `dismissed` is the stored `update.seen`.
    pub fn new(on: bool, now: Instant, dismissed: Option<Version>) -> Checker {
        Checker {
            next: (on && AVAILABLE).then(|| now + FIRST_CHECK),
            running: None,
            available: None,
            dismissed,
        }
    }

    /// Settings changed: turning the automatic check on checks soon, off stops it (a check on
    /// its way still answers).
    pub fn set_automatic(&mut self, on: bool, now: Instant) {
        match (on && AVAILABLE, self.next) {
            (true, None) => self.next = Some(now + FIRST_CHECK),
            (false, _) => self.next = None,
            (true, Some(_)) => {}
        }
    }

    pub fn automatic(&self) -> bool {
        self.next.is_some()
    }

    /// Whether an automatic check should start now; if so it counts as started.
    pub fn due(&mut self, now: Instant) -> bool {
        match self.next {
            Some(at) if now >= at && self.running.is_none() => {
                self.next = Some(now + INTERVAL);
                self.running = Some(Kind::Automatic);
                true
            }
            _ => false,
        }
    }

    /// "Check now": `true` to start a check; `false` when one is already on its way, which
    /// now answers as a manual one.
    pub fn start_manual(&mut self) -> bool {
        let start = self.running.is_none();
        self.running = Some(Kind::Manual);
        start
    }

    /// Whether a check is on its way.
    pub fn running(&self) -> Option<Kind> {
        self.running
    }

    /// A check's result: who asked, and the answer for `current` with `beta`. A check that
    /// finds nothing newer takes an earlier version down; a failure leaves it.
    pub fn finish(
        &mut self,
        result: Result<Manifest, String>,
        current: &Version,
        beta: bool,
    ) -> Finished {
        let kind = self.running.take().unwrap_or(Kind::Automatic);
        let answer = result.map(|m| m.newer(current, beta).cloned());
        if let Ok(v) = &answer {
            self.available = v.clone();
        }
        Finished { kind, answer }
    }

    /// The newer version the last check found.
    pub fn available(&self) -> Option<&Version> {
        self.available.as_ref()
    }

    /// The newer version to show as a banner: the available one, unless its banner (or a newer
    /// one's) was dismissed.
    pub fn banner(&self) -> Option<&Version> {
        let v = self.available.as_ref()?;
        match &self.dismissed {
            Some(d) if d >= v => None,
            _ => Some(v),
        }
    }

    /// "Later": hides the banner for the available version until a newer one comes.
    pub fn dismiss(&mut self) {
        if let Some(v) = &self.available
            && self.dismissed.as_ref().is_none_or(|d| v > d)
        {
            self.dismissed = Some(v.clone());
        }
    }

    /// The newest version whose banner was dismissed, to store as `update.seen`.
    pub fn dismissed(&self) -> Option<&Version> {
        self.dismissed.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(s: &str) -> Version {
        Version::parse(s).unwrap()
    }

    fn newer(s: &str) -> Result<Manifest, String> {
        Ok(Manifest {
            stable: Some(v(s)),
            prerelease: None,
        })
    }

    #[test]
    fn automatic_checks_and_the_banner() {
        let t0 = Instant::now();
        let current = v("1.0.0");
        let mut c = Checker::new(true, t0, None);
        if !AVAILABLE {
            assert!(!c.due(t0 + FIRST_CHECK));
            return;
        }
        assert!(c.automatic());
        assert!(!c.due(t0));
        assert!(c.due(t0 + FIRST_CHECK));
        assert_eq!(c.running(), Some(Kind::Automatic));
        // one at a time
        assert!(!c.due(t0 + FIRST_CHECK + INTERVAL));
        assert_eq!(
            c.finish(newer("1.0.1"), &current, false),
            Finished {
                kind: Kind::Automatic,
                answer: Ok(Some(v("1.0.1")))
            }
        );
        assert_eq!(c.running(), None);
        assert_eq!(c.available(), Some(&v("1.0.1")));
        assert_eq!(c.banner(), Some(&v("1.0.1")));
        // the next one an interval after the last started
        assert!(!c.due(t0 + FIRST_CHECK + INTERVAL - Duration::from_secs(1)));
        let t2 = t0 + FIRST_CHECK + INTERVAL;
        assert!(c.due(t2));
        c.finish(newer("1.0.1"), &current, false);
        // "Later" hides the banner for that version, across checks
        c.dismiss();
        assert_eq!(c.banner(), None);
        assert_eq!(c.dismissed(), Some(&v("1.0.1")));
        assert_eq!(c.available(), Some(&v("1.0.1")), "the card still shows it");
        assert!(c.due(t2 + INTERVAL));
        c.finish(newer("1.0.1"), &current, false);
        assert_eq!(c.banner(), None);
        // a newer one still: the banner is back
        assert!(c.due(t2 + INTERVAL * 2));
        c.finish(newer("1.0.2"), &current, false);
        assert_eq!(c.banner(), Some(&v("1.0.2")));
        // a failure leaves the version; nothing newer takes it down
        assert!(c.due(t2 + INTERVAL * 3));
        assert_eq!(
            c.finish(Err("timed out".into()), &current, false),
            Finished {
                kind: Kind::Automatic,
                answer: Err("timed out".into())
            }
        );
        assert_eq!(c.available(), Some(&v("1.0.2")));
        assert!(c.due(t2 + INTERVAL * 4));
        c.finish(Ok(Manifest::default()), &current, false);
        assert_eq!((c.available(), c.banner()), (None, None));
        c.dismiss();
        assert_eq!(c.dismissed(), Some(&v("1.0.1")), "nothing to dismiss");
    }

    #[test]
    fn the_dismissed_version_is_remembered_across_starts() {
        let t0 = Instant::now();
        let current = v("1.0.0");
        if !AVAILABLE {
            return;
        }
        let mut c = Checker::new(true, t0, Some(v("1.0.1")));
        assert!(c.due(t0 + FIRST_CHECK));
        c.finish(newer("1.0.1"), &current, false);
        assert_eq!(c.banner(), None);
        assert_eq!(c.available(), Some(&v("1.0.1")));
        // an older stored version does not cover a newer release
        let mut c = Checker::new(true, t0, Some(v("1.0.0-rc.1")));
        assert!(c.due(t0 + FIRST_CHECK));
        c.finish(newer("1.0.1"), &current, false);
        assert_eq!(c.banner(), Some(&v("1.0.1")));
    }

    #[test]
    fn manual_checks_and_switching_off() {
        let t0 = Instant::now();
        let current = v("1.0.0");
        let mut c = Checker::new(false, t0, None);
        assert!(!c.automatic());
        assert!(!c.due(t0 + INTERVAL * 2), "off");
        assert!(c.start_manual());
        assert!(!c.start_manual(), "one is on its way");
        assert_eq!(
            c.finish(newer("1.0.1"), &current, false),
            Finished {
                kind: Kind::Manual,
                answer: Ok(Some(v("1.0.1")))
            }
        );
        assert_eq!(c.available(), Some(&v("1.0.1")));
        if !AVAILABLE {
            return;
        }
        // switched on: soon; an automatic check on its way becomes manual
        c.set_automatic(true, t0);
        assert!(c.due(t0 + FIRST_CHECK));
        assert!(!c.start_manual());
        assert_eq!(
            c.finish(Ok(Manifest::default()), &current, false).kind,
            Kind::Manual
        );
        c.set_automatic(true, t0 + FIRST_CHECK);
        assert!(
            !c.due(t0 + FIRST_CHECK * 2),
            "switching on again keeps the schedule"
        );
        assert!(c.due(t0 + FIRST_CHECK + INTERVAL));
        c.set_automatic(false, t0);
        assert!(!c.automatic());
        assert!(!c.due(t0 + INTERVAL * 3));
    }
}
