//! Exercises during breaks (docs/EXERCISES.md): the catalogue, the planner
//! that builds a routine from the break's length and the time sat, and the
//! rotation that keeps routines varied. All pure; the engine owns one
//! [`ExerciseSetup`] and calls [`ExerciseSetup::plan`] when a break's nag is
//! raised or a break starts.

pub mod catalog;
pub mod planner;
pub mod rotation;

pub use catalog::{BUILTIN_TOML, Catalog, CatalogError, Exercise, Grade, Posture, Tag};
pub use planner::{PlanInput, plan};
pub use rotation::Rotation;

use ns_core::config::Exercises as ExercisesConfig;
use ns_core::{LocalDay, Routine};

/// The catalogue, the `[exercises]` config and the rotation state together.
#[derive(Clone, Debug, Default)]
pub struct ExerciseSetup {
    pub catalog: Catalog,
    pub config: ExercisesConfig,
    pub rotation: Rotation,
}

impl ExerciseSetup {
    pub fn new(catalog: Catalog, config: ExercisesConfig) -> Self {
        Self {
            catalog,
            config,
            rotation: Rotation::default(),
        }
    }

    /// Plans a routine; `None` when suggestions are off or nothing fits.
    pub fn plan(&self, input: &PlanInput) -> Option<Routine> {
        if !self.config.enabled {
            return None;
        }
        plan(&self.catalog, &self.config, &self.rotation, input)
    }

    /// Records a started break's routine for the rotation.
    pub fn note(&mut self, rule_id: &str, routine: &Routine, today: LocalDay) {
        let ids: Vec<String> = routine.steps.iter().map(|s| s.id.clone()).collect();
        self.rotation.note(rule_id, &ids, today);
    }
}
