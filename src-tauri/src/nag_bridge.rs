//! Turns engine nag events into toast and overlay windows.
//!
//! The only place where the tracking runtime and Tauri meet for nags: it
//! reads `NagEvent`s from the bus, keeps the list of open nags, opens or
//! closes windows to match, and pushes the list to them as `nags-changed`.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use ns_core::{Event, EventBus, NagEvent, NagLevel, Timestamp};
use ns_platform::Platform;
use ns_rules::EngineSnapshot;
use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tokio::sync::{broadcast, watch};

use crate::windows;

#[derive(Clone, Debug, Serialize)]
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
}

#[derive(Clone, Default)]
pub struct NagBridge {
    nags: Arc<Mutex<BTreeMap<String, NagView>>>,
    deferred: Arc<AtomicBool>,
}

impl NagBridge {
    pub fn current(&self) -> Vec<NagView> {
        self.nags.lock().unwrap().values().cloned().collect()
    }

    /// Applies one event; returns true if the list changed.
    fn apply(&self, event: &NagEvent, engine: &EngineSnapshot) -> bool {
        let mut nags = self.nags.lock().unwrap();
        match event {
            NagEvent::Raise {
                rule_id,
                rule_name,
                level,
                message,
                at,
                break_for_ms,
                snoozes_left,
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
                    },
                );
            }
            NagEvent::Escalate { rule_id, level, .. } => match nags.get_mut(rule_id) {
                Some(n) => n.level = *level,
                None => return false,
            },
            NagEvent::BreakStarted { rule_id, until } => {
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
                });
                entry.break_until = Some(*until);
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

    fn wants(&self) -> (bool, bool) {
        let nags = self.nags.lock().unwrap();
        let overlay = nags.values().any(|n| n.level >= NagLevel::Overlay);
        let toast = !overlay
            && nags
                .values()
                .any(|n| n.level == NagLevel::Toast && n.break_until.is_none());
        (toast, overlay)
    }

    /// Forwards bus events until the bus closes, and rechecks every second
    /// whether a deferred overlay can be shown. Runs on Tauri's runtime.
    pub fn spawn(
        &self,
        app: AppHandle,
        bus: &EventBus,
        engine: watch::Receiver<EngineSnapshot>,
        platform: Arc<dyn Platform>,
    ) {
        let bridge = self.clone();
        let mut rx = bus.subscribe();
        tauri::async_runtime::spawn(async move {
            let mut recheck = tokio::time::interval(Duration::from_secs(1));
            let mut deferred = false;
            let mut shown = (false, false);
            loop {
                tokio::select! {
                    event = rx.recv() => {
                        let nag = match event {
                            Ok(Event::Nag(n)) => n,
                            Ok(_) => continue,
                            Err(broadcast::error::RecvError::Lagged(_)) => continue,
                            Err(broadcast::error::RecvError::Closed) => break,
                        };
                        if !bridge.apply(&nag, &engine.borrow()) {
                            continue;
                        }
                        let _ = app.emit("nags-changed", bridge.current());
                    }
                    _ = recheck.tick() => {}
                }
                let (toast, overlay) = bridge.wants();
                let plan = plan_windows(toast, overlay, || platform.exclusive_fullscreen());
                if plan.deferred && !deferred {
                    tracing::info!("overlay deferred: exclusive fullscreen");
                    platform.alert();
                }
                deferred = plan.deferred;
                bridge.deferred.store(deferred, Ordering::SeqCst);
                if (plan.toast, plan.overlay) != shown {
                    match windows::sync_nag_windows(&app, plan.toast, plan.overlay) {
                        Ok(()) => shown = (plan.toast, plan.overlay),
                        Err(e) => tracing::warn!("cannot update nag windows: {e}"),
                    }
                }
            }
        });
    }

    /// True while a nag waits for an exclusive-fullscreen app to let go.
    pub fn is_deferred(&self) -> bool {
        self.deferred.load(Ordering::SeqCst)
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

#[cfg(test)]
mod tests {
    use ns_core::DismissReason;

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
        }
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
        let b = NagBridge::default();
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
                until: 10
            },
            &snap
        ));
        assert_eq!(b.current()[0].break_until, Some(10));
        assert!(b.apply(
            &NagEvent::Dismiss {
                rule_id: "a".into(),
                reason: DismissReason::BreakTaken,
                at: 10
            },
            &snap
        ));
        assert!(b.current().is_empty());
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
