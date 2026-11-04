//! The exercise catalogue: `config/exercises.toml` built in, plus the user's
//! `<data dir>/exercises.toml` merged by id.

use std::collections::HashSet;
use std::path::Path;

use ns_core::Equipment;
use serde::{Deserialize, Serialize};

/// The shipped catalogue (docs/EXERCISES.md).
pub const BUILTIN_TOML: &str = include_str!("../../../../config/exercises.toml");

/// Body region or purpose. The first tag of an item is its region, used to
/// keep one item per region in a routine.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tag {
    Eyes,
    Neck,
    Shoulders,
    UpperBack,
    Wrists,
    LowerBack,
    Hips,
    Legs,
    Walk,
    Cardio,
    Breathing,
    /// Not an exercise: drinking water, daylight. Suggested only with
    /// `[exercises] habits = true`, at most one per routine.
    Habits,
}

impl Tag {
    pub const ALL: [Tag; 12] = [
        Tag::Eyes,
        Tag::Neck,
        Tag::Shoulders,
        Tag::UpperBack,
        Tag::Wrists,
        Tag::LowerBack,
        Tag::Hips,
        Tag::Legs,
        Tag::Walk,
        Tag::Cardio,
        Tag::Breathing,
        Tag::Habits,
    ];

    /// Neck, shoulders and upper back.
    pub fn is_upper(self) -> bool {
        matches!(self, Tag::Neck | Tag::Shoulders | Tag::UpperBack)
    }

    /// Hips, legs, lower back, walking and cardio.
    pub fn is_lower(self) -> bool {
        matches!(
            self,
            Tag::Hips | Tag::Legs | Tag::LowerBack | Tag::Walk | Tag::Cardio
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Posture {
    Seated,
    Standing,
    Floor,
}

/// Strength of the evidence behind an item (docs/EXERCISES.md).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Grade {
    /// Pre/post or single lab study, low risk.
    C,
    /// One RCT or several small controlled trials.
    B,
    /// Meta-analysis or several RCTs, moderate certainty.
    A,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Exercise {
    pub id: String,
    pub name: String,
    pub tags: Vec<Tag>,
    pub posture: Posture,
    /// One set as written in `reps`; a walk's shortest length.
    pub seconds: u32,
    pub reps: String,
    #[serde(default)]
    pub equipment: Vec<Equipment>,
    pub cue: String,
    pub steps: Vec<String>,
    pub evidence: Grade,
    pub why: String,
    /// `false` in the user's file hides a built-in item.
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool {
    true
}

impl Exercise {
    /// The body region: the first tag.
    pub fn region(&self) -> Tag {
        self.tags[0]
    }

    pub fn has_tag(&self, tag: Tag) -> bool {
        self.tags.contains(&tag)
    }

    /// Walks stretch to the planner's length; everything else is fixed.
    pub fn is_walk(&self) -> bool {
        self.region() == Tag::Walk
    }

    /// Habits (water, daylight) rather than exercises.
    pub fn is_habit(&self) -> bool {
        self.region() == Tag::Habits
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CatalogError {
    #[error("invalid exercises: {0}")]
    Parse(#[from] toml::de::Error),
    #[error("exercise `{id}`: {msg}")]
    Invalid { id: String, msg: String },
    #[error("duplicate exercise id `{0}`")]
    DuplicateId(String),
    #[error("{path}: {msg}")]
    Io { path: String, msg: String },
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Catalog {
    #[serde(rename = "exercise", default)]
    pub exercises: Vec<Exercise>,
}

impl Catalog {
    pub fn from_toml(text: &str) -> Result<Self, CatalogError> {
        let cat: Catalog = toml::from_str(text)?;
        cat.validate()?;
        Ok(cat)
    }

    /// The shipped catalogue; a broken built-in file is a build error caught
    /// by the tests.
    pub fn builtin() -> Self {
        Self::from_toml(BUILTIN_TOML).expect("built-in exercises.toml is valid")
    }

    /// The built-in catalogue with the user's file merged in: same id
    /// replaces, new ids are appended, `enabled = false` hides. A missing
    /// file is fine.
    pub fn builtin_and_file(path: &Path) -> Result<Self, CatalogError> {
        let mut cat = Self::builtin();
        match std::fs::read_to_string(path) {
            Ok(text) => {
                let user: Catalog = toml::from_str(&text)?;
                for ex in user.exercises {
                    match cat.exercises.iter_mut().find(|e| e.id == ex.id) {
                        Some(slot) => *slot = ex,
                        None => cat.exercises.push(ex),
                    }
                }
                cat.validate()?;
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => {
                return Err(CatalogError::Io {
                    path: path.display().to_string(),
                    msg: e.to_string(),
                });
            }
        }
        Ok(cat)
    }

    pub fn get(&self, id: &str) -> Option<&Exercise> {
        self.exercises.iter().find(|e| e.id == id)
    }

    /// Enabled items only.
    pub fn enabled(&self) -> impl Iterator<Item = &Exercise> {
        self.exercises.iter().filter(|e| e.enabled)
    }

    fn validate(&self) -> Result<(), CatalogError> {
        let mut seen = HashSet::new();
        for e in &self.exercises {
            let invalid = |msg: &str| CatalogError::Invalid {
                id: e.id.clone(),
                msg: msg.to_string(),
            };
            if e.id.trim().is_empty() {
                return Err(invalid("id must not be empty"));
            }
            if !seen.insert(e.id.clone()) {
                return Err(CatalogError::DuplicateId(e.id.clone()));
            }
            if e.name.trim().is_empty() {
                return Err(invalid("name must not be empty"));
            }
            if e.tags.is_empty() {
                return Err(invalid("needs at least one tag"));
            }
            if e.seconds == 0 {
                return Err(invalid("seconds must be positive"));
            }
            if e.steps.is_empty() || e.steps.len() > 4 {
                return Err(invalid("steps must have 1 to 4 lines"));
            }
            if e.cue.trim().is_empty() {
                return Err(invalid("cue must not be empty"));
            }
            if e.posture == Posture::Floor && !e.equipment.contains(&Equipment::Floor) {
                return Err(invalid("floor items must list floor under equipment"));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_catalog_is_valid_and_complete() {
        let cat = Catalog::builtin();
        assert_eq!(cat.exercises.len(), 30);
        let free = cat
            .exercises
            .iter()
            .filter(|e| e.equipment.is_empty())
            .count();
        assert_eq!(free, 25);
        assert_eq!(cat.exercises.iter().filter(|e| e.is_habit()).count(), 2);
        assert!(cat.get("walk").unwrap().is_walk());
        for e in &cat.exercises {
            assert!(e.enabled, "{}", e.id);
            assert!(e.why.len() > 20, "{} needs a reason", e.id);
            assert!(!e.reps.is_empty(), "{}", e.id);
        }
        // Every region but cardio (stairs) has an item without equipment.
        for tag in Tag::ALL {
            if tag == Tag::Cardio {
                continue;
            }
            assert!(
                cat.exercises
                    .iter()
                    .any(|e| e.region() == tag && e.equipment.is_empty()),
                "no free item for {tag:?}"
            );
        }
        // Micro items fit a 30 s break.
        let micro: Vec<_> = cat
            .exercises
            .iter()
            .filter(|e| e.posture == Posture::Seated && e.seconds <= 30 && !e.is_habit())
            .collect();
        assert!(micro.len() >= 6, "{}", micro.len());
    }

    #[test]
    fn rejects_bad_items() {
        let dup = format!(
            "{BUILTIN_TOML}\n[[exercise]]\nid = \"walk\"\nname = \"x\"\ntags = [\"walk\"]\nposture = \"standing\"\nseconds = 10\nreps = \"1\"\ncue = \"c\"\nsteps = [\"s\"]\nevidence = \"C\"\nwhy = \"w\"\n"
        );
        assert!(matches!(
            Catalog::from_toml(&dup),
            Err(CatalogError::DuplicateId(id)) if id == "walk"
        ));
        let zero = "[[exercise]]\nid = \"z\"\nname = \"x\"\ntags = [\"eyes\"]\nposture = \"seated\"\nseconds = 0\nreps = \"1\"\ncue = \"c\"\nsteps = [\"s\"]\nevidence = \"C\"\nwhy = \"w\"\n";
        assert!(matches!(
            Catalog::from_toml(zero),
            Err(CatalogError::Invalid { msg, .. }) if msg.contains("seconds")
        ));
        assert!(
            Catalog::from_toml(
                "[[exercise]]
id = 1
"
            )
            .is_err()
        );
    }

    #[test]
    fn user_file_overrides_and_hides() {
        let dir = std::env::temp_dir().join(format!("ns-exercises-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("exercises.toml");
        std::fs::write(
            &path,
            r#"
[[exercise]]
id = "far-focus"
name = "Window gazing"
tags = ["eyes"]
posture = "seated"
seconds = 25
reps = "25 s"
cue = "c"
steps = ["look"]
evidence = "C"
why = "mine"

[[exercise]]
id = "chin-tuck"
name = "Chin tucks"
tags = ["neck"]
posture = "seated"
seconds = 30
reps = "8"
cue = "c"
steps = ["s"]
evidence = "B"
why = "w"
enabled = false

[[exercise]]
id = "my-stretch"
name = "My stretch"
tags = ["hips"]
posture = "standing"
seconds = 30
reps = "30 s"
cue = "c"
steps = ["s"]
evidence = "C"
why = "w"
"#,
        )
        .unwrap();
        let cat = Catalog::builtin_and_file(&path).unwrap();
        assert_eq!(cat.exercises.len(), 31);
        assert_eq!(cat.get("far-focus").unwrap().name, "Window gazing");
        assert!(!cat.get("chin-tuck").unwrap().enabled);
        assert!(cat.enabled().all(|e| e.id != "chin-tuck"));
        assert!(cat.get("my-stretch").is_some());
        assert_eq!(
            Catalog::builtin_and_file(&dir.join("missing.toml")).unwrap(),
            Catalog::builtin()
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
