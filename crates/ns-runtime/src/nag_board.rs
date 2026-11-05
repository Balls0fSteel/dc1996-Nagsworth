//! The open nags and the windows they need (docs/HEADLESS.md, phase 2).
//!
//! [`NagBoard`] is pure: it applies `NagEvent`s, seeds itself from the engine
//! snapshot, and says whether a toast or an overlay is wanted. [`run`] keeps
//! one on the runtime, defers overlays while an exclusive-fullscreen app owns
//! the screen (with a beep), and publishes a [`NagsView`] on a watch channel
//! and as `Event::Nags` whenever it changes. A GUI only opens and closes
//! windows to match.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use ns_core::{Event, EventBus, NagEvent, NagLevel, NagView, NagsView};
use ns_platform::Platform;
use ns_rules::{EngineSnapshot, NagState};
use tokio::sync::{broadcast, watch};

#[derive(Clone, Debug, Default)]
pub struct NagBoard {
    nags: BTreeMap<String, NagView>,
}

impl NagBoard {
    pub fn nags(&self) -> Vec<NagView> {
        self.nags.values().cloned().collect()
    }

    /// Applies one event; returns true if the list changed.
    pub fn apply(&mut self, event: &NagEvent, engine: &EngineSnapshot) -> bool {
        let nags = &mut self.nags;
        match event {
            NagEvent::Raise {
                rule_id,
                rule_name,
                level,
                message,
                at,
                break_for_ms,
                snoozes_left,
                routine,
            } => {
                nags.insert(
                    rule_id.clone(),
                    NagView {
                        rule_id: rule_id.clone(),
                        rule_name: rule_name.clone(),
                        message: message.clone(),
                        level: *level,
                        since: *at,
                        break_for_ms: *break_for_ms,
                        snoozes_left: *snoozes_left,
                        break_until: None,
                        routine: routine.clone(),
                    },
                );
            }
            NagEvent::Escalate { rule_id, level, .. } => match nags.get_mut(rule_id) {
                Some(n) => n.level = *level,
                None => return false,
            },
            NagEvent::BreakStarted {
                rule_id,
                until,
                routine,
            } => {
                let rule = engine.rules.iter().find(|r| &r.id == rule_id);
                let entry = nags.entry(rule_id.clone()).or_insert_with(|| NagView {
                    rule_id: rule_id.clone(),
                    rule_name: rule
                        .map(|r| r.name.clone())
                        .unwrap_or_else(|| rule_id.clone()),
                    message: rule.map(|r| r.message.clone()).unwrap_or_default(),
                    level: NagLevel::Overlay,
                    since: *until,
                    break_for_ms: None,
                    snoozes_left: 0,
                    break_until: None,
                    routine: None,
                });
                entry.break_until = Some(*until);
                if routine.is_some() {
                    entry.routine = routine.clone();
                }
                // A break started from the tray or dashboard was never raised,
                // so take the length from the snapshot for the progress ring.
                if entry.break_for_ms.is_none() {
                    entry.break_for_ms = rule
                        .map(|r| r.break_ms)
                        .filter(|b| *b > 0)
                        .map(|b| b as u64);
                }
                // Several break rules may start together; show one countdown.
                let longest = nags
                    .values()
                    .filter_map(|n| n.break_until.map(|u| (u, n.rule_id.clone())))
                    .max();
                if let Some((_, keep)) = longest {
                    for n in nags.values_mut().filter(|n| n.break_until.is_some()) {
                        n.level = if n.rule_id == keep {
                            NagLevel::Overlay
                        } else {
                            NagLevel::Toast
                        };
                    }
                }
            }
            NagEvent::Dismiss { rule_id, .. } => {
                if nags.remove(rule_id).is_none() {
                    return false;
                }
            }
        }
        true
    }

    /// Rebuilds the open nags from the engine snapshot. A rule may be nagging
    /// before the board subscribes (counters persisted across a restart), which would
    /// otherwise never get a window. Returns true if anything was added.
    pub fn seed(&mut self, engine: &EngineSnapshot) -> bool {
        let mut added = false;
        for rule in &engine.rules {
            let event = match rule.state {
                NagState::Active { level, since } => NagEvent::Raise {
                    rule_id: rule.id.clone(),
                    rule_name: rule.name.clone(),
                    level,
                    message: rule.message.clone(),
                    at: since,
                    break_for_ms: (rule.break_ms > 0).then_some(rule.break_ms as u64),
                    snoozes_left: rule.snoozes_left,
                    routine: rule.routine.clone(),
                },
                NagState::InBreak { until } => NagEvent::BreakStarted {
                    rule_id: rule.id.clone(),
                    until,
                    routine: rule.routine.clone(),
                },
                _ => continue,
            };
            added |= self.apply(&event, engine);
        }
        added
    }

    /// (toast, overlay) before deferral.
    pub fn wants(&self) -> (bool, bool) {
        let nags = &self.nags;
        let overlay = nags.values().any(|n| n.level >= NagLevel::Overlay);
        let toast = !overlay
            && nags
                .values()
                .any(|n| n.level == NagLevel::Toast && n.break_until.is_none());
        (toast, overlay)
    }
}

#[derive(Debug, PartialEq, Eq)]
struct WindowPlan {
    toast: bool,
    overlay: bool,
    deferred: bool,
}

/// Which windows to show. While an exclusive-fullscreen app owns the screen,
/// topmost windows stay invisible behind it (or knock it out of fullscreen),
/// so overlays wait; the caller beeps and the tray shows the nag instead.
fn plan_windows(toast: bool, overlay: bool, exclusive: impl FnOnce() -> bool) -> WindowPlan {
    if overlay && exclusive() {
        return WindowPlan {
            toast: false,
            overlay: false,
            deferred: true,
        };
    }
    WindowPlan {
        toast,
        overlay,
        deferred: false,
    }
}

/// Keeps a [`NagBoard`] in step with the bus until it closes, rechecking
/// every second whether a deferred overlay can be shown. `rx` is subscribed
/// before the engine starts.
pub async fn run(
    mut rx: broadcast::Receiver<Event>,
    bus: EventBus,
    engine: watch::Receiver<EngineSnapshot>,
    platform: Arc<dyn Platform>,
    out: watch::Sender<NagsView>,
) {
    let mut board = NagBoard::default();
    board.seed(&engine.borrow());
    let mut recheck = tokio::time::interval(Duration::from_secs(1));
    let mut last = NagsView::default();
    loop {
        tokio::select! {
            event = rx.recv() => {
                match event {
                    Ok(Event::Nag(n)) => {
                        if !board.apply(&n, &engine.borrow()) {
                            continue;
                        }
                    }
                    Ok(_) | Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
            _ = recheck.tick() => {}
        }
        let (toast, overlay) = board.wants();
        let p = plan_windows(toast, overlay, || platform.exclusive_fullscreen());
        if p.deferred && !last.deferred {
            tracing::info!("overlay deferred: exclusive fullscreen");
            platform.alert();
        }
        let view = NagsView {
            nags: board.nags(),
            toast: p.toast,
            overlay: p.overlay,
            deferred: p.deferred,
        };
        if view != last {
            out.send_replace(view.clone());
            bus.publish(Event::Nags(view.clone()));
            last = view;
        }
    }
}

#[cfg(test)]
mod tests {
    use ns_core::{DismissReason, Routine};

    use super::*;

    fn raise(id: &str, level: NagLevel) -> NagEvent {
        NagEvent::Raise {
            rule_id: id.into(),
            rule_name: id.into(),
            level,
            message: "m".into(),
            at: 1,
            break_for_ms: Some(1000),
            snoozes_left: 1,
            routine: None,
        }
    }

    #[test]
    fn seeds_from_a_snapshot_at_start() {
        use ns_rules::RuleSnapshot;
        let mut snap = EngineSnapshot::default();
        snap.rules.push(RuleSnapshot {
            id: "a".into(),
            name: "A".into(),
            trigger: "interval".into(),
            enabled: true,
            state: NagState::Active {
                level: NagLevel::Toast,
                since: 5,
            },
            accum_ms: 0,
            target_ms: 10,
            rest_ms: 0,
            break_ms: 600_000,
            snoozes_left: 2,
            message: "m".into(),
            idle_policy: ns_rules::IdlePolicy::Pause,
            routine: None,
        });
        snap.rules.push(RuleSnapshot {
            id: "b".into(),
            name: "B".into(),
            trigger: "interval".into(),
            enabled: true,
            state: NagState::InBreak { until: 99 },
            accum_ms: 0,
            target_ms: 10,
            rest_ms: 0,
            break_ms: 30_000,
            snoozes_left: 0,
            message: String::new(),
            idle_policy: ns_rules::IdlePolicy::Pause,
            routine: Some(Routine {
                steps: vec![ns_core::RoutineStep {
                    id: "walk".into(),
                    name: "Walk".into(),
                    seconds: 20,
                    reps: "20 s".into(),
                    cue: "c".into(),
                    lines: vec!["l".into()],
                }],
                note: None,
            }),
        });
        let mut b = NagBoard::default();
        assert!(b.seed(&snap));
        let nags = b.nags();
        assert_eq!(nags.len(), 2);
        let a = nags.iter().find(|n| n.rule_id == "a").unwrap();
        assert_eq!(
            (a.level, a.break_for_ms, a.snoozes_left),
            (NagLevel::Toast, Some(600_000), 2)
        );
        let brk = nags.iter().find(|n| n.rule_id == "b").unwrap();
        assert_eq!(
            (brk.break_until, brk.break_for_ms),
            (Some(99), Some(30_000))
        );
        assert_eq!(brk.routine.as_ref().unwrap().steps[0].id, "walk");
        // The running break takes the overlay; the toast rule waits.
        assert_eq!(b.wants(), (false, true));
        assert!(!NagBoard::default().seed(&EngineSnapshot::default()));
    }

    #[test]
    fn overlays_wait_for_exclusive_fullscreen() {
        let plan = plan_windows(false, true, || true);
        assert_eq!(
            plan,
            WindowPlan {
                toast: false,
                overlay: false,
                deferred: true
            }
        );
        assert!(plan_windows(false, true, || false).overlay);
        // Toasts never ask the platform.
        assert!(plan_windows(true, false, || panic!("not asked")).toast);
    }

    #[test]
    fn tracks_open_nags_and_wanted_windows() {
        let mut b = NagBoard::default();
        let snap = EngineSnapshot::default();
        assert_eq!(b.wants(), (false, false));
        assert!(b.apply(&raise("a", NagLevel::Toast), &snap));
        assert_eq!(b.wants(), (true, false));
        assert!(b.apply(
            &NagEvent::Escalate {
                rule_id: "a".into(),
                level: NagLevel::Overlay,
                at: 2
            },
            &snap
        ));
        assert_eq!(b.wants(), (false, true));
        assert!(b.apply(
            &NagEvent::BreakStarted {
                rule_id: "a".into(),
                until: 10,
                routine: None,
            },
            &snap
        ));
        assert_eq!(b.nags()[0].break_until, Some(10));
        assert!(b.apply(
            &NagEvent::Dismiss {
                rule_id: "a".into(),
                reason: DismissReason::BreakTaken,
                at: 10
            },
            &snap
        ));
        assert!(b.nags().is_empty());
        assert!(!b.apply(
            &NagEvent::Dismiss {
                rule_id: "a".into(),
                reason: DismissReason::BreakTaken,
                at: 10
            },
            &snap
        ));
    }
}
