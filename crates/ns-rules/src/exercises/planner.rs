//! Builds a routine for one break.
//!
//! Rules, in order (docs/EXERCISES.md "Picking a routine"):
//! 1. Budget: the steps add up to at most the break minus 10% for reading.
//!    Under a minute, exactly one item that fits the whole break.
//! 2. Walk length by time sat: under 30 min 1 min, under 60 min 2 min,
//!    under 90 min 3 min, otherwise 5 min, capped at 60% of the budget
//!    (Diaz 2023). Short breaks only walk after 30 min sat.
//! 3. One item per region; routines of 3+ items without a tag filter cover
//!    the upper body, the lower body and the eyes.
//! 4. Rotation: weight 1 + days since last shown (never shown = 8), halved
//!    for the previous break's items of the same rule; a micro item never
//!    repeats back to back. Seeded, so tests reproduce.
//! 5. Day load: after 4 h of activity lower-body items weigh double.
//! 6. Evidence first: grade A weighs 1.5x within a region.

use ns_core::config::Exercises as ExercisesConfig;
use ns_core::{LocalDay, Routine, RoutineStep};
use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};

use super::catalog::{Catalog, Exercise, Grade, Tag};
use super::rotation::Rotation;

/// Seconds of slack per 10 s of break, kept for reading the steps.
const SLACK_PER_TEN: i64 = 1;
const MAX_STEPS: usize = 6;
const MICRO_MS: i64 = 60_000;
const LONG_MS: i64 = 5 * 60_000;
const HEAVY_DAY_MS: i64 = 4 * 3_600_000;

pub struct PlanInput<'a> {
    pub rule_id: &'a str,
    /// The rule's `break_for`.
    pub break_ms: i64,
    /// Continuous time since the last rest that reset the rule.
    pub sat_ms: i64,
    /// Today's active time so far, any scope.
    pub today_active_ms: i64,
    /// The rule's tag filter; `None` for the whole catalogue.
    pub tags: Option<&'a [Tag]>,
    pub today: LocalDay,
    pub seed: u64,
}

/// Walk length for the time sat (Diaz 2023 dose), in seconds.
pub fn walk_seconds(sat_ms: i64) -> u32 {
    let min = sat_ms / 60_000;
    if min < 30 {
        60
    } else if min < 60 {
        120
    } else if min < 90 {
        180
    } else {
        300
    }
}

fn human(ms: i64) -> String {
    let min = ms / 60_000;
    let (h, m) = (min / 60, min % 60);
    if h > 0 {
        format!("{h} h {m:02} min")
    } else {
        format!("{m} min")
    }
}

fn step(e: &Exercise, seconds: u32) -> RoutineStep {
    RoutineStep {
        id: e.id.clone(),
        name: e.name.clone(),
        seconds,
        reps: e.reps.clone(),
        cue: e.cue.clone(),
        lines: e.steps.clone(),
    }
}

struct Picker<'a> {
    rotation: &'a Rotation,
    input: &'a PlanInput<'a>,
    rng: SmallRng,
}

impl Picker<'_> {
    fn weight(&self, e: &Exercise) -> f64 {
        let days = self
            .rotation
            .days_since(&e.id, self.input.today)
            .map(|d| d.min(7) as f64)
            .unwrap_or(7.0);
        let mut w = 1.0 + days;
        if e.evidence == Grade::A {
            w *= 1.5;
        }
        if self.input.today_active_ms > HEAVY_DAY_MS && e.region().is_lower() {
            w *= 2.0;
        }
        if self.rotation.was_last_for(self.input.rule_id, &e.id) {
            w *= 0.5;
        }
        w
    }

    fn pick<'c>(&mut self, candidates: &[&'c Exercise]) -> Option<&'c Exercise> {
        if candidates.is_empty() {
            return None;
        }
        let weights: Vec<f64> = candidates.iter().map(|e| self.weight(e)).collect();
        let total: f64 = weights.iter().sum();
        let mut r = self.rng.random::<f64>() * total;
        for (e, w) in candidates.iter().zip(&weights) {
            if r < *w {
                return Some(e);
            }
            r -= w;
        }
        candidates.last().copied()
    }
}

pub fn plan(
    catalog: &Catalog,
    config: &ExercisesConfig,
    rotation: &Rotation,
    input: &PlanInput,
) -> Option<Routine> {
    let break_s = (input.break_ms / 1000).max(0);
    if break_s < 15 {
        return None;
    }
    let eligible: Vec<&Exercise> = catalog
        .enabled()
        .filter(|e| !config.exclude.iter().any(|x| x == &e.id))
        .filter(|e| e.equipment.iter().all(|q| config.equipment.contains(q)))
        .filter(|e| !(e.is_walk() && !config.walks))
        .filter(|e| match input.tags {
            Some(tags) => e.tags.iter().any(|t| tags.contains(t)),
            None => true,
        })
        .collect();
    let mut picker = Picker {
        rotation,
        input,
        rng: SmallRng::seed_from_u64(input.seed),
    };

    if input.break_ms < MICRO_MS {
        let fits: Vec<&Exercise> = eligible
            .iter()
            .copied()
            .filter(|e| !e.is_walk() && i64::from(e.seconds) <= break_s)
            .collect();
        let fresh: Vec<&Exercise> = fits
            .iter()
            .copied()
            .filter(|e| !rotation.was_last_for(input.rule_id, &e.id))
            .collect();
        let e = picker.pick(if fresh.is_empty() { &fits } else { &fresh })?;
        return Some(Routine {
            steps: vec![step(e, e.seconds)],
            note: None,
        });
    }

    let budget = break_s - break_s / 10 * SLACK_PER_TEN;
    let mut left = budget;
    let mut steps: Vec<RoutineStep> = Vec::new();
    let mut regions: Vec<Tag> = Vec::new();
    let mut note = None;

    // The walk: always in long breaks, in short ones after 30 min sat.
    let long = input.break_ms >= LONG_MS;
    if let Some(walk) = eligible.iter().copied().find(|e| e.is_walk())
        && (long || input.sat_ms >= 30 * 60_000)
    {
        let cap = (budget * 6 / 10).max(i64::from(walk.seconds));
        let secs = i64::from(walk_seconds(input.sat_ms)).min(cap).min(left);
        if secs >= i64::from(walk.seconds) {
            steps.push(step(walk, secs as u32));
            regions.push(Tag::Walk);
            left -= secs;
            if input.sat_ms >= 60 * 60_000 {
                note = Some(format!(
                    "{} without a real break: walk first.",
                    human(input.sat_ms)
                ));
            }
        }
    }

    // Breathing closes long untargeted routines; reserve its time.
    let breathing = (long && input.tags.is_none())
        .then(|| {
            eligible
                .iter()
                .copied()
                .filter(|e| e.region() == Tag::Breathing && i64::from(e.seconds) <= left)
                .collect::<Vec<_>>()
        })
        .and_then(|c| picker.pick(&c));
    if let Some(b) = breathing {
        left -= i64::from(b.seconds);
    }

    let candidates = |left: i64, regions: &[Tag], filter: &dyn Fn(&Exercise) -> bool| {
        eligible
            .iter()
            .copied()
            .filter(|e| !e.is_walk() && e.region() != Tag::Breathing)
            .filter(|e| i64::from(e.seconds) <= left && !regions.contains(&e.region()))
            .filter(|e| filter(e))
            .collect::<Vec<_>>()
    };

    // Required coverage for routines of three or more items.
    if input.tags.is_none() && input.break_ms >= 3 * 60_000 {
        let groups: [&dyn Fn(&Exercise) -> bool; 3] = [
            &|e| e.region().is_upper(),
            &|e| e.region().is_lower(),
            &|e| e.region() == Tag::Eyes,
        ];
        for (i, group) in groups.iter().enumerate() {
            if i == 1 && regions.contains(&Tag::Walk) {
                continue;
            }
            let c = candidates(left, &regions, group);
            if let Some(e) = picker.pick(&c) {
                steps.push(step(e, e.seconds));
                regions.push(e.region());
                left -= i64::from(e.seconds);
            }
        }
    }

    // Fill what is left.
    while steps.len() + usize::from(breathing.is_some()) < MAX_STEPS {
        let c = candidates(left, &regions, &|_| true);
        let Some(e) = picker.pick(&c) else { break };
        steps.push(step(e, e.seconds));
        regions.push(e.region());
        left -= i64::from(e.seconds);
    }

    if let Some(b) = breathing {
        steps.push(step(b, b.seconds));
    }
    if steps.is_empty() {
        return None;
    }
    Some(Routine { steps, note })
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use ns_core::Equipment;

    use super::*;

    fn day() -> LocalDay {
        "2026-11-04".parse().unwrap()
    }

    fn input<'a>(break_ms: i64, sat_ms: i64, tags: Option<&'a [Tag]>) -> PlanInput<'a> {
        PlanInput {
            rule_id: "r",
            break_ms,
            sat_ms,
            today_active_ms: 0,
            tags,
            today: day(),
            seed: 1,
        }
    }

    fn plan_default(i: &PlanInput) -> Routine {
        plan(
            &Catalog::builtin(),
            &ExercisesConfig::default(),
            &Rotation::default(),
            i,
        )
        .unwrap()
    }

    #[test]
    fn micro_break_is_one_item_that_fits() {
        let r = plan_default(&input(30_000, 20 * 60_000, None));
        assert_eq!(r.steps.len(), 1);
        assert!(r.steps[0].seconds <= 30);
        assert!(r.note.is_none());
        let eyes = [Tag::Eyes];
        for seed in 0..20 {
            let mut i = input(30_000, 0, Some(&eyes));
            i.seed = seed;
            let r = plan_default(&i);
            assert!(
                ["far-focus", "blink-squeeze", "near-far"].contains(&r.steps[0].id.as_str()),
                "{}",
                r.steps[0].id
            );
        }
    }

    #[test]
    fn two_minute_move_break_walks_after_thirty_minutes() {
        let tags = [Tag::Walk, Tag::Legs, Tag::Hips];
        let r = plan_default(&input(120_000, 30 * 60_000, Some(&tags)));
        assert_eq!(r.steps[0].id, "walk");
        // 2 min walk by the dose, capped at 60% of the 108 s budget.
        assert_eq!(r.steps[0].seconds, 64);
        assert!(r.total_seconds() <= 108);
        assert!(r.steps.iter().all(|s| s.id != "cyclic-sigh"));
        // Before 30 min sat: no walk, legs or hips items instead.
        let r = plan_default(&input(120_000, 10 * 60_000, Some(&tags)));
        assert!(r.steps.iter().all(|s| s.id != "walk"));
        assert!(!r.steps.is_empty());
    }

    #[test]
    fn walk_scales_with_time_sat() {
        assert_eq!(walk_seconds(20 * 60_000), 60);
        assert_eq!(walk_seconds(45 * 60_000), 120);
        assert_eq!(walk_seconds(75 * 60_000), 180);
        assert_eq!(walk_seconds(120 * 60_000), 300);
        let r = plan_default(&input(600_000, 120 * 60_000, None));
        assert_eq!(r.steps[0].id, "walk");
        assert_eq!(r.steps[0].seconds, 300);
        assert_eq!(
            r.note.as_deref(),
            Some("2 h 00 min without a real break: walk first.")
        );
        let r = plan_default(&input(600_000, 45 * 60_000, None));
        assert_eq!(r.steps[0].seconds, 120);
        assert!(r.note.is_none());
    }

    #[test]
    fn long_routine_is_balanced_and_fits() {
        for seed in 0..30 {
            let mut i = input(600_000, 60 * 60_000, None);
            i.seed = seed;
            let r = plan_default(&i);
            assert!(
                r.total_seconds() <= 540,
                "seed {seed}: {}",
                r.total_seconds()
            );
            assert!(r.steps.len() >= 4 && r.steps.len() <= 6, "seed {seed}");
            assert_eq!(r.steps[0].id, "walk");
            assert_eq!(r.steps.last().unwrap().id, "cyclic-sigh");
            let cat = Catalog::builtin();
            let regions: Vec<Tag> = r
                .steps
                .iter()
                .map(|s| cat.get(&s.id).unwrap().region())
                .collect();
            let unique: HashSet<_> = regions.iter().collect();
            assert_eq!(unique.len(), regions.len(), "seed {seed}: {regions:?}");
            assert!(regions.iter().any(|t| t.is_upper()), "seed {seed}");
            assert!(regions.contains(&Tag::Eyes), "seed {seed}");
            let ids: HashSet<_> = r.steps.iter().map(|s| &s.id).collect();
            assert_eq!(ids.len(), r.steps.len());
        }
    }

    #[test]
    fn flags_exclusions_and_walks_off_are_respected() {
        let cat = Catalog::builtin();
        let mut cfg = ExercisesConfig {
            exclude: vec!["far-focus".into(), "walk".into()],
            ..ExercisesConfig::default()
        };
        for seed in 0..40 {
            let mut i = input(600_000, 90 * 60_000, None);
            i.seed = seed;
            let r = plan(&cat, &cfg, &Rotation::default(), &i).unwrap();
            for s in &r.steps {
                assert!(!cfg.exclude.contains(&s.id), "{}", s.id);
                assert!(cat.get(&s.id).unwrap().equipment.is_empty(), "{}", s.id);
            }
        }
        cfg.exclude.clear();
        cfg.walks = false;
        cfg.equipment = vec![Equipment::Band, Equipment::Floor, Equipment::Stairs];
        let mut seen = HashSet::new();
        for seed in 0..60 {
            let mut i = input(600_000, 90 * 60_000, None);
            i.seed = seed;
            let r = plan(&cat, &cfg, &Rotation::default(), &i).unwrap();
            assert!(r.steps.iter().all(|s| s.id != "walk"));
            seen.extend(r.steps.iter().map(|s| s.id.clone()));
        }
        assert!(seen.contains("band-pull-apart") || seen.contains("band-row"));
        assert!(seen.contains("glute-bridge") || seen.contains("bird-dog"));
    }

    #[test]
    fn rotation_avoids_repeats_and_is_deterministic() {
        let cat = Catalog::builtin();
        let cfg = ExercisesConfig::default();
        let eyes = [Tag::Eyes];
        let i = input(30_000, 0, Some(&eyes));
        let a = plan(&cat, &cfg, &Rotation::default(), &i).unwrap();
        let b = plan(&cat, &cfg, &Rotation::default(), &i).unwrap();
        assert_eq!(a, b);
        let mut rot = Rotation::default();
        let mut last = a.steps[0].id.clone();
        for seed in 0..30 {
            rot.note("r", &[last.clone()], day());
            let mut i = input(30_000, 0, Some(&eyes));
            i.seed = seed;
            let r = plan(&cat, &cfg, &rot, &i).unwrap();
            assert_ne!(r.steps[0].id, last, "seed {seed}");
            last = r.steps[0].id.clone();
        }
    }

    #[test]
    fn nothing_fits_or_disabled() {
        let cat = Catalog::builtin();
        let cfg = ExercisesConfig::default();
        assert!(plan(&cat, &cfg, &Rotation::default(), &input(5_000, 0, None)).is_none());
        let band = [Tag::Breathing];
        let mut cfg2 = cfg.clone();
        cfg2.exclude = vec!["cyclic-sigh".into()];
        assert!(
            plan(
                &cat,
                &cfg2,
                &Rotation::default(),
                &input(60_000, 0, Some(&band))
            )
            .is_none()
        );
        let setup = super::super::ExerciseSetup {
            catalog: cat,
            config: ExercisesConfig {
                enabled: false,
                ..cfg
            },
            rotation: Rotation::default(),
        };
        assert!(setup.plan(&input(600_000, 0, None)).is_none());
    }

    #[test]
    fn heavy_day_favours_the_lower_body() {
        let cat = Catalog::builtin();
        let cfg = ExercisesConfig::default();
        let count = |heavy: bool| {
            (0..200)
                .filter(|seed| {
                    let mut i = input(45_000, 0, None);
                    i.seed = *seed;
                    i.today_active_ms = if heavy { 5 * 3_600_000 } else { 0 };
                    let r = plan(&cat, &cfg, &Rotation::default(), &i).unwrap();
                    cat.get(&r.steps[0].id).unwrap().region().is_lower()
                })
                .count()
        };
        assert!(count(true) > count(false));
    }
}
