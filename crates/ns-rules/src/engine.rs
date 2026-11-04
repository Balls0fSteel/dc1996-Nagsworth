//! The nag engine: per-rule counters and a small state machine.
//!
//! ```text
//! Idle --threshold reached--> Active{level} --escalation--> Active{higher level}
//! Active --snooze--> Snoozed{until} --until passed, still due--> Active
//! Active --skip--> Idle (break rules) | Satisfied{since} (limits, schedules)
//! Active --start break--> InBreak{until} --until passed--> Idle
//! Active --enough rest / left scope--> Idle | Satisfied
//! Satisfied --renag interval passed, still due--> Active
//! ```

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::time::Duration;

use ns_core::config::Exercises as ExercisesConfig;
use ns_core::{
    Activity, ControlCommand, DismissReason, LocalDay, NagEvent, NagLevel, Routine, SegmentKind,
    TimeOfDay, Timestamp,
};
use serde::{Deserialize, Serialize};

use crate::exercises::{ExerciseSetup, PlanInput};
use crate::rule::{IdlePolicy, Rule, RuleSet, Trigger};

/// Gaps between ticks longer than this are rest, not activity.
const MAX_TICK_MS: i64 = 5_000;
/// How long a schedule nag waits after a skip before nagging again.
const SCHEDULE_RENAG_MS: i64 = 5 * 60_000;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum NagState {
    #[default]
    Idle,
    /// Due, but waiting for in-scope activity before showing anything.
    Pending,
    Active {
        level: NagLevel,
        since: Timestamp,
    },
    Snoozed {
        until: Timestamp,
    },
    InBreak {
        until: Timestamp,
    },
    /// Dismissed while still over a limit; nags again after the renag interval.
    Satisfied {
        since: Timestamp,
    },
}

/// Counters that survive restarts.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Counters {
    pub accum_ms: i64,
    pub rest_ms: i64,
    pub snoozes_used: u32,
    pub day: Option<LocalDay>,
}

#[derive(Clone, Debug, Default)]
struct RuleState {
    counters: Counters,
    nag: NagState,
    /// The exercises planned for the open nag or the running break.
    routine: Option<Routine>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuleSnapshot {
    pub id: String,
    pub name: String,
    pub trigger: String,
    pub enabled: bool,
    pub state: NagState,
    /// Counted time toward the threshold.
    pub accum_ms: i64,
    /// The threshold (interval, limit), 0 for schedules.
    pub target_ms: i64,
    pub rest_ms: i64,
    /// Rest needed to reset, 0 for daily limits and schedules.
    pub break_ms: i64,
    pub snoozes_left: u32,
    pub message: String,
    /// The policy in effect: the rule's own or its default, after reading mode.
    pub idle_policy: IdlePolicy,
    /// The exercises of the open nag or the running break.
    pub routine: Option<Routine>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EngineSnapshot {
    pub at: Timestamp,
    pub reading_mode: bool,
    /// `tracking.hands_on_games`: Game rules count hands-on time by default.
    pub hands_on_games: bool,
    pub rules: Vec<RuleSnapshot>,
}

#[derive(Debug)]
pub struct Engine {
    rules: RuleSet,
    states: HashMap<String, RuleState>,
    reading_mode: bool,
    hands_on_games: bool,
    day_start: TimeOfDay,
    last_tick: Option<Timestamp>,
    /// Exercise suggestions; `None` without a catalogue (tests).
    exercises: Option<ExerciseSetup>,
    /// Today's active time in any scope, for the planner's day load.
    day_active_ms: i64,
    active_day: Option<LocalDay>,
}

fn ms(d: Duration) -> i64 {
    d.as_millis() as i64
}

/// Plans the exercises for `rule`'s break: the rule's `break_for`, the time
/// counted since its last reset, and today's load.
fn plan_routine(
    exercises: &Option<ExerciseSetup>,
    day_active_ms: i64,
    rule: &Rule,
    st: &RuleState,
    now: Timestamp,
    day: LocalDay,
) -> Option<Routine> {
    let setup = exercises.as_ref()?;
    if rule.exercises == crate::rule::RuleExercises::Off {
        return None;
    }
    let break_for = match &rule.trigger {
        Trigger::Interval { break_for, .. } => *break_for,
        Trigger::Continuous { reset_after, .. } => *reset_after,
        _ => return None,
    };
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    rule.id.hash(&mut hasher);
    now.hash(&mut hasher);
    setup.plan(&PlanInput {
        rule_id: &rule.id,
        break_ms: ms(break_for),
        sat_ms: st.counters.accum_ms,
        today_active_ms: day_active_ms,
        tags: rule.exercises.tags(),
        today: day,
        seed: hasher.finish(),
    })
}

impl Engine {
    pub fn new(
        rules: RuleSet,
        day_start: TimeOfDay,
        reading_mode: bool,
        hands_on_games: bool,
    ) -> Self {
        let mut e = Self {
            rules: RuleSet::default(),
            states: HashMap::new(),
            reading_mode,
            hands_on_games,
            day_start,
            last_tick: None,
            exercises: None,
            day_active_ms: 0,
            active_day: None,
        };
        e.replace_rules(rules, 0);
        e
    }

    /// Turns on exercise suggestions with this catalogue and config.
    pub fn with_exercises(mut self, setup: ExerciseSetup) -> Self {
        self.exercises = Some(setup);
        self
    }

    /// Applies a changed `[exercises]` config; the catalogue and rotation stay.
    pub fn set_exercises_config(&mut self, config: ExercisesConfig) {
        if let Some(setup) = &mut self.exercises {
            setup.config = config;
        }
    }

    /// Restores the rotation state (JSON from [`Engine::export_rotation`]).
    pub fn import_rotation(&mut self, json: &str) {
        if let (Some(setup), Some(rot)) = (
            &mut self.exercises,
            crate::exercises::Rotation::from_json(json),
        ) {
            setup.rotation = rot;
        }
    }

    pub fn export_rotation(&self) -> Option<String> {
        self.exercises.as_ref().map(|s| s.rotation.to_json())
    }

    pub fn rules(&self) -> &RuleSet {
        &self.rules
    }

    pub fn reading_mode(&self) -> bool {
        self.reading_mode
    }

    pub fn hands_on_games(&self) -> bool {
        self.hands_on_games
    }

    /// Switches the default policy of Game rules between `input` and `ignore`.
    /// Counters are kept.
    pub fn set_hands_on_games(&mut self, on: bool) {
        self.hands_on_games = on;
    }

    /// The policy `rule` counts with right now. Reading mode turns `pause`
    /// into `ignore` (break timers keep running while reading) but leaves
    /// `input` alone: game budgets still count only hands-on time.
    fn effective_policy(&self, rule: &Rule) -> IdlePolicy {
        match rule.idle_policy(self.hands_on_games) {
            IdlePolicy::Pause if self.reading_mode => IdlePolicy::Ignore,
            p => p,
        }
    }

    /// Swaps in a new rule set. Rules that keep their id keep their counters;
    /// open nags are dismissed so the UI starts clean.
    pub fn replace_rules(&mut self, rules: RuleSet, now: Timestamp) -> Vec<NagEvent> {
        let mut out = Vec::new();
        for (id, st) in &mut self.states {
            if matches!(st.nag, NagState::Active { .. }) {
                out.push(NagEvent::Dismiss {
                    rule_id: id.clone(),
                    reason: DismissReason::Reload,
                    at: now,
                });
            }
            st.nag = NagState::Idle;
        }
        self.states
            .retain(|id, _| rules.rules.iter().any(|r| &r.id == id));
        for r in &rules.rules {
            self.states.entry(r.id.clone()).or_default();
        }
        self.rules = rules;
        out
    }

    /// Restores persisted counters (JSON from [`Engine::export_counters`]).
    pub fn import_counters(&mut self, saved: &[(String, String)]) {
        for (id, json) in saved {
            if let (Some(st), Ok(c)) = (
                self.states.get_mut(id),
                serde_json::from_str::<Counters>(json),
            ) {
                st.counters = c;
            }
        }
    }

    pub fn export_counters(&self) -> Vec<(String, String)> {
        self.states
            .iter()
            .filter_map(|(id, st)| Some((id.clone(), serde_json::to_string(&st.counters).ok()?)))
            .collect()
    }

    fn raise(
        rule: &Rule,
        st: &mut RuleState,
        now: Timestamp,
        routine: Option<Routine>,
        out: &mut Vec<NagEvent>,
    ) {
        let level = rule.level_at(Duration::ZERO);
        st.nag = NagState::Active { level, since: now };
        st.routine = routine.clone();
        let break_for_ms = match &rule.trigger {
            Trigger::Interval { break_for, .. } => Some(break_for.as_millis() as u64),
            Trigger::Continuous { reset_after, .. } => Some(reset_after.as_millis() as u64),
            _ => None,
        };
        out.push(NagEvent::Raise {
            rule_id: rule.id.clone(),
            rule_name: rule.display_name().to_string(),
            level,
            message: rule.message(),
            at: now,
            break_for_ms,
            snoozes_left: rule
                .snooze()
                .max_per_day
                .saturating_sub(st.counters.snoozes_used),
            routine,
        });
    }

    fn dismiss(
        rule: &Rule,
        st: &mut RuleState,
        next: NagState,
        reason: DismissReason,
        now: Timestamp,
        out: &mut Vec<NagEvent>,
    ) {
        if matches!(st.nag, NagState::Active { .. }) {
            out.push(NagEvent::Dismiss {
                rule_id: rule.id.clone(),
                reason,
                at: now,
            });
        }
        // A snoozed nag comes back with the same routine.
        if !matches!(next, NagState::Snoozed { .. }) {
            st.routine = None;
        }
        st.nag = next;
    }

    /// Advances all rules to `now` given what the user is doing.
    pub fn tick(&mut self, now: Timestamp, offset_s: i32, activity: &Activity) -> Vec<NagEvent> {
        let raw_dt = self.last_tick.map(|t| now - t).unwrap_or(0).max(0);
        self.last_tick = Some(now);
        // A long gap (sleep, stall) is rest, never activity.
        let gap = raw_dt > MAX_TICK_MS;
        let dt = if gap { 0 } else { raw_dt };
        let day = LocalDay::from_ts(now, offset_s, self.day_start);
        let time_of_day = TimeOfDay::of(now, offset_s);
        let mut out = Vec::new();

        if self.active_day != Some(day) {
            self.active_day = Some(day);
            self.day_active_ms = 0;
        }
        if activity.kind == Some(SegmentKind::Active) && !gap {
            self.day_active_ms += dt;
        }

        for rule in &self.rules.rules {
            let policy = self.effective_policy(rule);
            let st = self.states.entry(rule.id.clone()).or_default();
            if !rule.enabled {
                Self::dismiss(
                    rule,
                    st,
                    NagState::Idle,
                    DismissReason::Reload,
                    now,
                    &mut out,
                );
                continue;
            }
            if st.counters.day != Some(day) {
                st.counters.day = Some(day);
                st.counters.snoozes_used = 0;
                if matches!(rule.trigger, Trigger::DailyTotal { .. }) {
                    st.counters.accum_ms = 0;
                    Self::dismiss(
                        rule,
                        st,
                        NagState::Idle,
                        DismissReason::Resolved,
                        now,
                        &mut out,
                    );
                }
            }

            let in_scope = rule.scope.matches(activity.app.as_ref());
            let engaged = match activity.kind {
                Some(SegmentKind::Active) => policy != IdlePolicy::Input || activity.hands_on,
                Some(SegmentKind::Idle) => {
                    policy == IdlePolicy::Ignore
                        || (policy == IdlePolicy::Input && activity.hands_on)
                }
                _ => false,
            };
            let counting = in_scope && engaged;
            let rest_dt = if gap { raw_dt } else { dt };

            // Breaks in progress just run out.
            if let NagState::InBreak { until } = st.nag {
                if now >= until {
                    st.counters.accum_ms = 0;
                    st.counters.rest_ms = 0;
                    st.nag = NagState::Idle;
                    st.routine = None;
                    out.push(NagEvent::Dismiss {
                        rule_id: rule.id.clone(),
                        reason: DismissReason::BreakTaken,
                        at: now,
                    });
                }
                continue;
            }

            let snooze_over = matches!(st.nag, NagState::Snoozed { until } if now >= until);
            match &rule.trigger {
                Trigger::Interval {
                    every,
                    break_for: rest_needed,
                }
                | Trigger::Continuous {
                    limit: every,
                    reset_after: rest_needed,
                } => {
                    if counting && !gap {
                        st.counters.accum_ms += dt;
                        st.counters.rest_ms = 0;
                    } else {
                        st.counters.rest_ms += rest_dt;
                    }
                    if st.counters.rest_ms >= ms(*rest_needed) {
                        st.counters.accum_ms = 0;
                        if st.nag != NagState::Idle {
                            Self::dismiss(
                                rule,
                                st,
                                NagState::Idle,
                                DismissReason::BreakTaken,
                                now,
                                &mut out,
                            );
                        }
                    }
                    let due = st.counters.accum_ms >= ms(*every);
                    // Continuous nags only while the scope is in use.
                    let can_show = counting || matches!(rule.trigger, Trigger::Interval { .. });
                    if due && can_show && (st.nag == NagState::Idle || snooze_over) {
                        let routine = st.routine.take().or_else(|| {
                            plan_routine(&self.exercises, self.day_active_ms, rule, st, now, day)
                        });
                        Self::raise(rule, st, now, routine, &mut out);
                    }
                }
                Trigger::DailyTotal { limit, renag_every } => {
                    if counting {
                        st.counters.accum_ms += dt;
                    }
                    let due = st.counters.accum_ms >= ms(*limit);
                    let renag = matches!(st.nag, NagState::Satisfied { since }
                        if now - since >= ms(*renag_every));
                    if due && st.nag == NagState::Idle {
                        st.nag = NagState::Pending;
                    }
                    if counting && (st.nag == NagState::Pending || snooze_over || renag) {
                        Self::raise(rule, st, now, None, &mut out);
                    } else if !in_scope && matches!(st.nag, NagState::Active { .. }) {
                        Self::dismiss(
                            rule,
                            st,
                            NagState::Satisfied { since: now },
                            DismissReason::Resolved,
                            now,
                            &mut out,
                        );
                    }
                }
                Trigger::Schedule { windows } => {
                    let in_window = windows.iter().any(|w| w.contains(time_of_day));
                    if !in_window {
                        Self::dismiss(
                            rule,
                            st,
                            NagState::Idle,
                            DismissReason::Resolved,
                            now,
                            &mut out,
                        );
                    } else {
                        let renag = matches!(st.nag, NagState::Satisfied { since }
                            if now - since >= SCHEDULE_RENAG_MS);
                        if counting && (st.nag == NagState::Idle || snooze_over || renag) {
                            Self::raise(rule, st, now, None, &mut out);
                        } else if !in_scope && matches!(st.nag, NagState::Active { .. }) {
                            Self::dismiss(
                                rule,
                                st,
                                NagState::Idle,
                                DismissReason::Resolved,
                                now,
                                &mut out,
                            );
                        }
                    }
                }
            }

            // Escalate open nags, but only while the user keeps going.
            if let NagState::Active { level, since } = st.nag
                && counting
            {
                let due = rule.level_at(Duration::from_millis((now - since).max(0) as u64));
                if due > level {
                    st.nag = NagState::Active { level: due, since };
                    out.push(NagEvent::Escalate {
                        rule_id: rule.id.clone(),
                        level: due,
                        at: now,
                    });
                }
            }
        }
        out
    }

    pub fn command(&mut self, cmd: &ControlCommand, now: Timestamp) -> Vec<NagEvent> {
        let mut out = Vec::new();
        match cmd {
            ControlCommand::Snooze { rule_id } => {
                if let (Some(rule), Some(st)) =
                    (self.rules.get(rule_id), self.states.get_mut(rule_id))
                {
                    let snooze = rule.snooze();
                    if matches!(st.nag, NagState::Active { .. })
                        && st.counters.snoozes_used < snooze.max_per_day
                    {
                        st.counters.snoozes_used += 1;
                        Self::dismiss(
                            rule,
                            st,
                            NagState::Snoozed {
                                until: now + ms(snooze.duration),
                            },
                            DismissReason::Snoozed,
                            now,
                            &mut out,
                        );
                    }
                }
            }
            ControlCommand::SkipBreak { rule_id } => {
                // During a started break, skipping ends it early: the rest so
                // far still resets the rule, but the break is not counted as
                // taken (completion tracking, docs/EXERCISES.md).
                if let (Some(rule), Some(st)) =
                    (self.rules.get(rule_id), self.states.get_mut(rule_id))
                    && matches!(st.nag, NagState::InBreak { .. })
                {
                    st.counters.accum_ms = 0;
                    st.counters.rest_ms = 0;
                    st.nag = NagState::Idle;
                    st.routine = None;
                    out.push(NagEvent::Dismiss {
                        rule_id: rule.id.clone(),
                        reason: DismissReason::EndedEarly,
                        at: now,
                    });
                } else if let (Some(rule), Some(st)) =
                    (self.rules.get(rule_id), self.states.get_mut(rule_id))
                    && matches!(st.nag, NagState::Active { .. } | NagState::Snoozed { .. })
                {
                    let next = match rule.trigger {
                        Trigger::Interval { .. } | Trigger::Continuous { .. } => {
                            st.counters.accum_ms = 0;
                            NagState::Idle
                        }
                        _ => NagState::Satisfied { since: now },
                    };
                    out.push(NagEvent::Dismiss {
                        rule_id: rule.id.clone(),
                        reason: DismissReason::Skipped,
                        at: now,
                    });
                    st.nag = next;
                    st.routine = None;
                }
            }
            ControlCommand::StartBreakNow { rule_id } => {
                let day = self
                    .active_day
                    .unwrap_or_else(|| LocalDay::from_ts(now, 0, self.day_start));
                for rule in &self.rules.rules {
                    if rule_id.as_ref().is_some_and(|id| *id != rule.id) || !rule.enabled {
                        continue;
                    }
                    let length = match rule.trigger {
                        Trigger::Interval { break_for, .. } => break_for,
                        Trigger::Continuous { reset_after, .. } => reset_after,
                        _ => continue,
                    };
                    let st = self.states.entry(rule.id.clone()).or_default();
                    let until = now + ms(length);
                    // The nag's routine if there was one, else a fresh plan.
                    let routine = st.routine.take().or_else(|| {
                        plan_routine(&self.exercises, self.day_active_ms, rule, st, now, day)
                    });
                    if let (Some(setup), Some(r)) = (&mut self.exercises, &routine) {
                        setup.note(&rule.id, r, day);
                    }
                    st.nag = NagState::InBreak { until };
                    st.routine = routine.clone();
                    out.push(NagEvent::BreakStarted {
                        rule_id: rule.id.clone(),
                        until,
                        routine,
                    });
                }
            }
            ControlCommand::SetReadingMode { on } => self.reading_mode = *on,
            ControlCommand::PauseTracking { .. }
            | ControlCommand::ResumeTracking
            | ControlCommand::Reload => {}
        }
        out
    }

    pub fn snapshot(&self, now: Timestamp) -> EngineSnapshot {
        let rules = self
            .rules
            .rules
            .iter()
            .map(|r| {
                let st = self.states.get(&r.id).cloned().unwrap_or_default();
                let (target, brk) = match &r.trigger {
                    Trigger::Interval { every, break_for } => (ms(*every), ms(*break_for)),
                    Trigger::Continuous { limit, reset_after } => (ms(*limit), ms(*reset_after)),
                    Trigger::DailyTotal { limit, .. } => (ms(*limit), 0),
                    Trigger::Schedule { .. } => (0, 0),
                };
                RuleSnapshot {
                    id: r.id.clone(),
                    name: r.display_name().to_string(),
                    trigger: r.trigger.kind().to_string(),
                    enabled: r.enabled,
                    state: st.nag,
                    accum_ms: st.counters.accum_ms,
                    target_ms: target,
                    rest_ms: st.counters.rest_ms,
                    break_ms: brk,
                    snoozes_left: r
                        .snooze()
                        .max_per_day
                        .saturating_sub(st.counters.snoozes_used),
                    message: r.message(),
                    idle_policy: self.effective_policy(r),
                    routine: st.routine,
                }
            })
            .collect();
        EngineSnapshot {
            at: now,
            reading_mode: self.reading_mode,
            hands_on_games: self.hands_on_games,
            rules,
        }
    }
}

#[cfg(test)]
mod tests {
    use ns_core::{AppRef, CategoryKind};

    use super::*;

    fn rules(toml: &str) -> RuleSet {
        RuleSet::from_toml(toml).unwrap()
    }

    fn act(exe: &str, cat: CategoryKind, kind: SegmentKind) -> Activity {
        Activity {
            kind: Some(kind),
            app: Some(AppRef {
                id: 1,
                exe_name: exe.into(),
                category: cat,
                is_game: cat == CategoryKind::Game,
            }),
            ..Activity::default()
        }
    }

    /// Ticks once per second from `from` for `secs` seconds.
    fn run(e: &mut Engine, from: Timestamp, secs: i64, a: &Activity) -> Vec<NagEvent> {
        (0..secs)
            .flat_map(|i| e.tick(from + i * 1000, 0, a))
            .collect()
    }

    const INTERVAL: &str = r#"
        [[rules]]
        id = "brk"
        trigger = { kind = "interval", every = "60s", break_for = "10s" }
        escalation = [ { after = "0s", level = "toast" }, { after = "5s", level = "overlay" } ]
        snooze = { duration = "20s", max_per_day = 1 }
    "#;

    #[test]
    fn interval_raises_escalates_and_resets_on_rest() {
        let mut e = Engine::new(rules(INTERVAL), TimeOfDay::MIDNIGHT, false, false);
        let work = act("code.exe", CategoryKind::Work, SegmentKind::Active);
        // 60 s of activity -> raise at t = 60 s (first tick has dt 0).
        let ev = run(&mut e, 0, 61, &work);
        assert!(matches!(
            ev.as_slice(),
            [NagEvent::Raise {
                level: NagLevel::Toast,
                at: 60_000,
                ..
            }]
        ));
        let ev = run(&mut e, 61_000, 5, &work);
        assert!(matches!(
            ev.as_slice(),
            [NagEvent::Escalate {
                level: NagLevel::Overlay,
                ..
            }]
        ));
        // 10 s idle (pause policy) is a full break.
        let idle = act("code.exe", CategoryKind::Work, SegmentKind::Idle);
        let ev = run(&mut e, 66_000, 11, &idle);
        assert!(matches!(
            ev.as_slice(),
            [NagEvent::Dismiss {
                reason: DismissReason::BreakTaken,
                ..
            }]
        ));
        assert_eq!(e.snapshot(0).rules[0].accum_ms, 0);
    }

    #[test]
    fn snooze_limits_and_skip() {
        let mut e = Engine::new(rules(INTERVAL), TimeOfDay::MIDNIGHT, false, false);
        let work = act("code.exe", CategoryKind::Work, SegmentKind::Active);
        run(&mut e, 0, 61, &work);
        let ev = e.command(
            &ControlCommand::Snooze {
                rule_id: "brk".into(),
            },
            61_000,
        );
        assert!(matches!(
            ev.as_slice(),
            [NagEvent::Dismiss {
                reason: DismissReason::Snoozed,
                ..
            }]
        ));
        // Re-raised once the snooze runs out.
        let ev = run(&mut e, 61_000, 21, &work);
        assert!(matches!(
            ev.as_slice(),
            [NagEvent::Raise {
                snoozes_left: 0,
                ..
            }]
        ));
        // No snoozes left.
        assert!(
            e.command(
                &ControlCommand::Snooze {
                    rule_id: "brk".into()
                },
                82_000
            )
            .is_empty()
        );
        let ev = e.command(
            &ControlCommand::SkipBreak {
                rule_id: "brk".into(),
            },
            82_000,
        );
        assert!(matches!(
            ev.as_slice(),
            [NagEvent::Dismiss {
                reason: DismissReason::Skipped,
                ..
            }]
        ));
        assert_eq!(e.snapshot(0).rules[0].state, NagState::Idle);
    }

    #[test]
    fn started_break_runs_out() {
        let mut e = Engine::new(rules(INTERVAL), TimeOfDay::MIDNIGHT, false, false);
        let work = act("code.exe", CategoryKind::Work, SegmentKind::Active);
        run(&mut e, 0, 61, &work);
        let ev = e.command(&ControlCommand::StartBreakNow { rule_id: None }, 61_000);
        assert_eq!(
            ev,
            vec![NagEvent::BreakStarted {
                rule_id: "brk".into(),
                until: 71_000,
                routine: None,
            }]
        );
        let ev = run(&mut e, 62_000, 10, &work);
        assert!(matches!(
            ev.as_slice(),
            [NagEvent::Dismiss {
                reason: DismissReason::BreakTaken,
                at: 71_000,
                ..
            }]
        ));
    }

    #[test]
    fn started_break_can_end_early() {
        let mut e = Engine::new(rules(INTERVAL), TimeOfDay::MIDNIGHT, false, false);
        let work = act("code.exe", CategoryKind::Work, SegmentKind::Active);
        run(&mut e, 0, 61, &work);
        e.command(&ControlCommand::StartBreakNow { rule_id: None }, 61_000);
        let ev = e.command(
            &ControlCommand::SkipBreak {
                rule_id: "brk".into(),
            },
            64_000,
        );
        assert_eq!(
            ev,
            vec![NagEvent::Dismiss {
                rule_id: "brk".into(),
                reason: DismissReason::EndedEarly,
                at: 64_000,
            }]
        );
        let snap = e.snapshot(64_000);
        assert_eq!(snap.rules[0].state, NagState::Idle);
        assert_eq!(snap.rules[0].accum_ms, 0);
        // Nothing left to run out.
        assert!(run(&mut e, 65_000, 10, &work).is_empty());
    }

    #[test]
    fn reading_mode_keeps_counting_while_idle() {
        let mut e = Engine::new(rules(INTERVAL), TimeOfDay::MIDNIGHT, true, false);
        let idle = act("reader.exe", CategoryKind::Other, SegmentKind::Idle);
        let ev = run(&mut e, 0, 61, &idle);
        assert!(matches!(ev.as_slice(), [NagEvent::Raise { .. }]));
        // Locked still counts as rest.
        let locked = Activity {
            kind: Some(SegmentKind::Locked),
            ..Activity::default()
        };
        let ev = run(&mut e, 61_000, 11, &locked);
        assert!(matches!(ev.as_slice(), [NagEvent::Dismiss { .. }]));
    }

    const DAILY: &str = r#"
        [[rules]]
        id = "games"
        scope = { category = "Game" }
        trigger = { kind = "daily_total", limit = "30s", renag_every = "20s" }
    "#;

    #[test]
    fn daily_limit_counts_only_scope_and_renags() {
        let mut e = Engine::new(rules(DAILY), TimeOfDay::MIDNIGHT, false, false);
        let game = act("game.exe", CategoryKind::Game, SegmentKind::Active);
        let work = act("code.exe", CategoryKind::Work, SegmentKind::Active);
        assert!(run(&mut e, 0, 100, &work).is_empty());
        // Gamepad play shows as idle; Game scope ignores idle by default.
        let pad = act("game.exe", CategoryKind::Game, SegmentKind::Idle);
        let ev = run(&mut e, 100_000, 31, &pad);
        assert!(matches!(ev.as_slice(), [NagEvent::Raise { .. }]), "{ev:?}");
        // Switching away resolves the nag; coming back within renag does not re-raise.
        let ev = run(&mut e, 131_000, 5, &work);
        assert!(matches!(
            ev.as_slice(),
            [NagEvent::Dismiss {
                reason: DismissReason::Resolved,
                ..
            }]
        ));
        assert!(run(&mut e, 136_000, 5, &game).is_empty());
        let ev = run(&mut e, 141_000, 20, &game);
        assert!(matches!(ev.as_slice(), [NagEvent::Raise { .. }]));
        // A new day resets the total.
        let ev = e.tick(86_400_000 + 1000, 0, &game);
        assert!(matches!(ev.as_slice(), [NagEvent::Dismiss { .. }]));
        assert_eq!(e.snapshot(0).rules[0].accum_ms, 0);
    }

    #[test]
    fn continuous_resets_after_rest() {
        let set = rules(
            r#"
            [[rules]]
            id = "c"
            scope = { app = "game.exe" }
            trigger = { kind = "continuous", limit = "20s", reset_after = "5s" }
        "#,
        );
        let mut e = Engine::new(set, TimeOfDay::MIDNIGHT, false, false);
        let game = act("game.exe", CategoryKind::Game, SegmentKind::Active);
        let other = act("x.exe", CategoryKind::Other, SegmentKind::Active);
        run(&mut e, 0, 15, &game);
        run(&mut e, 15_000, 3, &other); // short pause: no reset
        assert!(e.snapshot(0).rules[0].accum_ms > 10_000);
        run(&mut e, 18_000, 6, &other); // long pause: reset
        assert_eq!(e.snapshot(0).rules[0].accum_ms, 0);
        let ev = run(&mut e, 24_000, 22, &game);
        assert!(matches!(ev.as_slice(), [NagEvent::Raise { .. }]));
    }

    #[test]
    fn schedule_nags_inside_window_only() {
        let set = rules(
            r#"
            [[rules]]
            id = "late"
            scope = { category = "Game" }
            trigger = { kind = "schedule", windows = ["23:00-06:00"] }
            escalation = [ { after = "0s", level = "overlay" } ]
        "#,
        );
        let mut e = Engine::new(set, TimeOfDay::MIDNIGHT, false, false);
        let game = act("game.exe", CategoryKind::Game, SegmentKind::Active);
        let noon = 12 * 3_600_000;
        assert!(run(&mut e, noon, 5, &game).is_empty());
        // 23:00 local with a +1h offset is 22:00 UTC.
        let ev = e.tick(22 * 3_600_000, 3600, &game);
        assert!(matches!(
            ev.as_slice(),
            [NagEvent::Raise {
                level: NagLevel::Overlay,
                ..
            }]
        ));
        let ev = e.tick(
            22 * 3_600_000 + 1000,
            3600,
            &act("x.exe", CategoryKind::Work, SegmentKind::Active),
        );
        assert!(matches!(ev.as_slice(), [NagEvent::Dismiss { .. }]));
    }

    #[test]
    fn counters_persist_and_reload_dismisses() {
        let mut e = Engine::new(rules(DAILY), TimeOfDay::MIDNIGHT, false, false);
        let game = act("game.exe", CategoryKind::Game, SegmentKind::Active);
        run(&mut e, 0, 31, &game);
        let saved = e.export_counters();
        let ev = e.replace_rules(rules(DAILY), 31_000);
        assert!(matches!(
            ev.as_slice(),
            [NagEvent::Dismiss {
                reason: DismissReason::Reload,
                ..
            }]
        ));

        let mut fresh = Engine::new(rules(DAILY), TimeOfDay::MIDNIGHT, false, false);
        fresh.import_counters(&saved);
        assert_eq!(fresh.snapshot(0).rules[0].accum_ms, 30_000);
        // Still over the limit after a restart on the same day.
        let ev = fresh.tick(32_000, 0, &game);
        assert!(matches!(ev.as_slice(), [NagEvent::Raise { .. }]));
    }

    fn hands(mut a: Activity, on: bool) -> Activity {
        a.hands_on = on;
        a
    }

    const DAILY_LONG: &str = r#"
        [[rules]]
        id = "games"
        scope = { category = "Game" }
        trigger = { kind = "daily_total", limit = "1h" }
    "#;

    /// 10 hands-on ticks and 10 without input (half active, half idle).
    fn mixed_game_ticks(e: &mut Engine) {
        let game = act("game.exe", CategoryKind::Game, SegmentKind::Active);
        let pad = act("game.exe", CategoryKind::Game, SegmentKind::Idle);
        e.tick(0, 0, &hands(game.clone(), true));
        run(e, 1000, 5, &hands(game.clone(), true));
        run(e, 6000, 5, &hands(pad.clone(), true));
        run(e, 11_000, 5, &hands(game, false));
        run(e, 16_000, 5, &hands(pad, false));
    }

    #[test]
    fn game_rules_count_hands_on_time_by_default() {
        let mut e = Engine::new(rules(DAILY_LONG), TimeOfDay::MIDNIGHT, false, true);
        mixed_game_ticks(&mut e);
        let snap = e.snapshot(0);
        assert!(snap.hands_on_games);
        assert_eq!(snap.rules[0].idle_policy, IdlePolicy::Input);
        assert_eq!(snap.rules[0].accum_ms, 10_000);
    }

    #[test]
    fn game_rules_count_all_time_in_front_without_hands_on_games() {
        let mut e = Engine::new(rules(DAILY_LONG), TimeOfDay::MIDNIGHT, false, false);
        mixed_game_ticks(&mut e);
        let snap = e.snapshot(0);
        assert!(!snap.hands_on_games);
        assert_eq!(snap.rules[0].idle_policy, IdlePolicy::Ignore);
        assert_eq!(snap.rules[0].accum_ms, 20_000);
        // The switch applies from the next tick on and keeps the counters.
        e.set_hands_on_games(true);
        let game = act("game.exe", CategoryKind::Game, SegmentKind::Active);
        run(&mut e, 21_000, 5, &hands(game, false));
        assert_eq!(e.snapshot(0).rules[0].accum_ms, 20_000);
    }

    const INPUT_INTERVAL: &str = r#"
        [[rules]]
        id = "hands"
        idle_policy = "input"
        scope = { category = "Work" }
        trigger = { kind = "interval", every = "60s", break_for = "10s" }
    "#;

    #[test]
    fn input_policy_counts_hands_on_and_rests_otherwise() {
        let mut e = Engine::new(rules(INPUT_INTERVAL), TimeOfDay::MIDNIGHT, false, false);
        let work = act("code.exe", CategoryKind::Work, SegmentKind::Active);
        let idle = act("code.exe", CategoryKind::Work, SegmentKind::Idle);
        e.tick(0, 0, &hands(work.clone(), true));
        run(&mut e, 1000, 30, &hands(work.clone(), true));
        assert_eq!(e.snapshot(0).rules[0].accum_ms, 30_000);
        // Watching without input (active or idle) is rest, not activity.
        run(&mut e, 31_000, 5, &hands(work.clone(), false));
        run(&mut e, 36_000, 4, &hands(idle.clone(), false));
        let snap = e.snapshot(0);
        assert_eq!(snap.rules[0].accum_ms, 30_000);
        assert_eq!(snap.rules[0].rest_ms, 9_000);
        // Idle with recent input (a gamepad, say) still counts.
        run(&mut e, 40_000, 5, &hands(idle, true));
        let snap = e.snapshot(0);
        assert_eq!(snap.rules[0].accum_ms, 35_000);
        assert_eq!(snap.rules[0].rest_ms, 0);
        // 10 s without input is a full break.
        run(&mut e, 45_000, 10, &hands(work, false));
        assert_eq!(e.snapshot(0).rules[0].accum_ms, 0);
    }

    #[test]
    fn reading_mode_leaves_input_rules_alone() {
        let mut e = Engine::new(rules(INPUT_INTERVAL), TimeOfDay::MIDNIGHT, true, false);
        let idle = act("code.exe", CategoryKind::Work, SegmentKind::Idle);
        run(&mut e, 0, 20, &hands(idle, false));
        let snap = e.snapshot(0);
        assert_eq!(snap.rules[0].idle_policy, IdlePolicy::Input);
        assert_eq!(snap.rules[0].accum_ms, 0);
        assert_eq!(snap.rules[0].rest_ms, 19_000);
        // A pause rule does turn into ignore.
        let e = Engine::new(rules(INTERVAL), TimeOfDay::MIDNIGHT, true, false);
        assert_eq!(e.snapshot(0).rules[0].idle_policy, IdlePolicy::Ignore);
    }

    #[test]
    fn sleep_gap_counts_as_rest() {
        let mut e = Engine::new(rules(INTERVAL), TimeOfDay::MIDNIGHT, false, false);
        let work = act("code.exe", CategoryKind::Work, SegmentKind::Active);
        run(&mut e, 0, 30, &work);
        // 1 h later: the gap is rest, so the timer resets.
        e.tick(3_600_000, 0, &work);
        assert_eq!(e.snapshot(0).rules[0].accum_ms, 0);
    }
}
