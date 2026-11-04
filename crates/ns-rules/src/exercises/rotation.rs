//! Which exercises were shown when, so routines vary from day to day.
//! Persisted as JSON in `settings` (`exercises.rotation`); losing it only
//! changes variety.

use std::collections::BTreeMap;

use ns_core::LocalDay;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rotation {
    /// The day each exercise was last part of a started break.
    #[serde(default)]
    pub last_shown: BTreeMap<String, LocalDay>,
    /// The exercises of the last started break, per rule.
    #[serde(default)]
    pub last_by_rule: BTreeMap<String, Vec<String>>,
}

impl Rotation {
    pub fn from_json(json: &str) -> Option<Self> {
        serde_json::from_str(json).ok()
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    /// Days since `id` was last shown, capped; `None` if never.
    pub fn days_since(&self, id: &str, today: LocalDay) -> Option<u32> {
        self.last_shown
            .get(id)
            .map(|d| today.days_since(*d).clamp(0, 30) as u32)
    }

    pub fn was_last_for(&self, rule_id: &str, id: &str) -> bool {
        self.last_by_rule
            .get(rule_id)
            .is_some_and(|ids| ids.iter().any(|i| i == id))
    }

    /// Records a started break's exercises.
    pub fn note(&mut self, rule_id: &str, ids: &[String], today: LocalDay) {
        for id in ids {
            self.last_shown.insert(id.clone(), today);
        }
        self.last_by_rule.insert(rule_id.to_string(), ids.to_vec());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notes_and_round_trips() {
        let d1: LocalDay = "2026-11-04".parse().unwrap();
        let d2: LocalDay = "2026-11-06".parse().unwrap();
        let mut r = Rotation::default();
        assert_eq!(r.days_since("walk", d2), None);
        r.note("rest", &["walk".into(), "squats".into()], d1);
        assert_eq!(r.days_since("walk", d2), Some(2));
        assert!(r.was_last_for("rest", "squats"));
        assert!(!r.was_last_for("micro", "squats"));
        let back = Rotation::from_json(&r.to_json()).unwrap();
        assert_eq!(back, r);
        assert!(Rotation::from_json("not json").is_none());
    }
}
