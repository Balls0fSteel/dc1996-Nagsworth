//! Rule definitions as written in the config file.

use std::collections::HashSet;
use std::str::FromStr;
use std::time::Duration;

use ns_core::{AppRef, CategoryKind, NagLevel, TimeOfDay};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Debug, thiserror::Error)]
pub enum RuleError {
    #[error("invalid rules: {0}")]
    Parse(#[from] toml::de::Error),
    #[error("rule `{id}`: {msg}")]
    Invalid { id: String, msg: String },
    #[error("duplicate rule id `{0}`")]
    DuplicateId(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Trigger {
    /// Nag after `every` of activity; `break_for` of rest resets the timer.
    Interval {
        #[serde(with = "humantime_serde")]
        every: Duration,
        #[serde(with = "humantime_serde")]
        break_for: Duration,
    },
    /// Nag once `limit` of in-scope time is reached on a local day.
    DailyTotal {
        #[serde(with = "humantime_serde")]
        limit: Duration,
        #[serde(with = "humantime_serde", default = "default_renag")]
        renag_every: Duration,
    },
    /// Nag after `limit` of uninterrupted in-scope time; `reset_after` of
    /// rest resets it.
    Continuous {
        #[serde(with = "humantime_serde")]
        limit: Duration,
        #[serde(with = "humantime_serde")]
        reset_after: Duration,
    },
    /// Nag whenever the scope is active inside one of the windows.
    Schedule { windows: Vec<ScheduleWindow> },
}

fn default_renag() -> Duration {
    Duration::from_secs(15 * 60)
}

impl Trigger {
    pub fn kind(&self) -> &'static str {
        match self {
            Trigger::Interval { .. } => "interval",
            Trigger::DailyTotal { .. } => "daily_total",
            Trigger::Continuous { .. } => "continuous",
            Trigger::Schedule { .. } => "schedule",
        }
    }
}

/// `HH:MM-HH:MM`; wraps past midnight when the end is before the start.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScheduleWindow {
    pub start: TimeOfDay,
    pub end: TimeOfDay,
}

impl ScheduleWindow {
    pub fn contains(&self, t: TimeOfDay) -> bool {
        if self.start <= self.end {
            self.start <= t && t < self.end
        } else {
            t >= self.start || t < self.end
        }
    }
}

impl FromStr for ScheduleWindow {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (a, b) = s
            .split_once('-')
            .ok_or_else(|| format!("invalid window `{s}`, expected HH:MM-HH:MM"))?;
        Ok(Self {
            start: a.parse().map_err(|e| format!("{e}"))?,
            end: b.parse().map_err(|e| format!("{e}"))?,
        })
    }
}

impl Serialize for ScheduleWindow {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(&format_args!("{}-{}", self.start, self.end))
    }
}

impl<'de> Deserialize<'de> for ScheduleWindow {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        String::deserialize(d)?
            .parse()
            .map_err(serde::de::Error::custom)
    }
}

/// Which activity a rule watches.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Scope {
    #[default]
    Any,
    Category(CategoryKind),
    App(String),
}

impl Scope {
    pub fn matches(&self, app: Option<&AppRef>) -> bool {
        match self {
            Scope::Any => true,
            Scope::Category(c) => app.is_some_and(|a| a.category == *c),
            Scope::App(exe) => app.is_some_and(|a| a.exe_name.eq_ignore_ascii_case(exe)),
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum ScopeRepr {
    Name(String),
    Category { category: CategoryKind },
    App { app: String },
}

impl Serialize for Scope {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            Scope::Any => ScopeRepr::Name("any".into()),
            Scope::Category(c) => ScopeRepr::Category { category: *c },
            Scope::App(a) => ScopeRepr::App { app: a.clone() },
        }
        .serialize(s)
    }
}

impl<'de> Deserialize<'de> for Scope {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        match ScopeRepr::deserialize(d)? {
            ScopeRepr::Name(n) if n.eq_ignore_ascii_case("any") => Ok(Scope::Any),
            ScopeRepr::Name(n) => Err(serde::de::Error::custom(format!(
                "unknown scope `{n}`, expected \"any\", {{ category = ... }} or {{ app = ... }}"
            ))),
            ScopeRepr::Category { category } => Ok(Scope::Category(category)),
            ScopeRepr::App { app } => Ok(Scope::App(app)),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdlePolicy {
    /// Idle time does not count and counts as rest.
    Pause,
    /// Idle time counts as activity (reading, watching, gamepad play).
    Ignore,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EscalationStep {
    #[serde(with = "humantime_serde")]
    pub after: Duration,
    pub level: NagLevel,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snooze {
    #[serde(with = "humantime_serde")]
    pub duration: Duration,
    pub max_per_day: u32,
}

impl Default for Snooze {
    fn default() -> Self {
        Self {
            duration: Duration::from_secs(5 * 60),
            max_per_day: 3,
        }
    }
}

fn default_true() -> bool {
    true
}

fn default_escalation() -> Vec<EscalationStep> {
    vec![EscalationStep {
        after: Duration::ZERO,
        level: NagLevel::Toast,
    }]
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rule {
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub scope: Scope,
    pub trigger: Trigger,
    /// Defaults to `ignore` for the Game category (gamepad input is invisible
    /// to idle detection) and `pause` otherwise.
    #[serde(default)]
    pub idle_policy: Option<IdlePolicy>,
    #[serde(default = "default_escalation")]
    pub escalation: Vec<EscalationStep>,
    #[serde(default)]
    pub snooze: Option<Snooze>,
    /// Text shown in the nag; a default is derived from the trigger.
    #[serde(default)]
    pub message: Option<String>,
}

impl Rule {
    pub fn display_name(&self) -> &str {
        self.name.as_deref().unwrap_or(&self.id)
    }

    pub fn idle_policy(&self) -> IdlePolicy {
        self.idle_policy.unwrap_or(match self.scope {
            Scope::Category(CategoryKind::Game) => IdlePolicy::Ignore,
            _ => IdlePolicy::Pause,
        })
    }

    pub fn snooze(&self) -> Snooze {
        self.snooze.clone().unwrap_or_default()
    }

    pub fn message(&self) -> String {
        if let Some(m) = &self.message {
            return m.clone();
        }
        let d = |d: &Duration| humantime::format_duration(*d).to_string();
        match &self.trigger {
            Trigger::Interval { every, break_for } => format!(
                "You have been at it for {}. Take a {} break.",
                d(every),
                d(break_for)
            ),
            Trigger::DailyTotal { limit, .. } => {
                format!("You reached today's limit of {}.", d(limit))
            }
            Trigger::Continuous { limit, reset_after } => format!(
                "{} without a pause. Step away for {}.",
                d(limit),
                d(reset_after)
            ),
            Trigger::Schedule { .. } => "This is outside your allowed hours.".to_string(),
        }
    }

    /// The highest escalation level due `elapsed` after the nag was raised.
    pub fn level_at(&self, elapsed: Duration) -> NagLevel {
        self.escalation
            .iter()
            .filter(|s| s.after <= elapsed)
            .map(|s| s.level)
            .max()
            .unwrap_or(NagLevel::Toast)
    }

    fn validate(&self) -> Result<(), RuleError> {
        let invalid = |msg: &str| RuleError::Invalid {
            id: self.id.clone(),
            msg: msg.to_string(),
        };
        if self.id.trim().is_empty() {
            return Err(invalid("id must not be empty"));
        }
        if self.escalation.is_empty() {
            return Err(invalid("escalation must have at least one step"));
        }
        match &self.trigger {
            Trigger::Interval { every, break_for } => {
                if every.is_zero() || break_for.is_zero() {
                    return Err(invalid("every and break_for must be positive"));
                }
            }
            Trigger::DailyTotal { limit, renag_every } => {
                if limit.is_zero() || renag_every.is_zero() {
                    return Err(invalid("limit and renag_every must be positive"));
                }
            }
            Trigger::Continuous { limit, reset_after } => {
                if limit.is_zero() || reset_after.is_zero() {
                    return Err(invalid("limit and reset_after must be positive"));
                }
            }
            Trigger::Schedule { windows } => {
                if windows.is_empty() {
                    return Err(invalid("schedule needs at least one window"));
                }
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuleSet {
    #[serde(default)]
    pub rules: Vec<Rule>,
}

impl RuleSet {
    /// Parses the `[[rules]]` tables of a config file; other tables are ignored.
    pub fn from_toml(text: &str) -> Result<Self, RuleError> {
        let set: RuleSet = toml::from_str(text)?;
        let mut seen = HashSet::new();
        for r in &set.rules {
            r.validate()?;
            if !seen.insert(r.id.clone()) {
                return Err(RuleError::DuplicateId(r.id.clone()));
            }
        }
        Ok(set)
    }

    pub fn get(&self, id: &str) -> Option<&Rule> {
        self.rules.iter().find(|r| r.id == id)
    }
}

#[cfg(test)]
mod tests {
    use ns_core::config::DEFAULT_CONFIG_TOML;

    use super::*;

    #[test]
    fn parses_the_shipped_example() {
        let set = RuleSet::from_toml(DEFAULT_CONFIG_TOML).unwrap();
        assert_eq!(set.rules.len(), 4);
        let rest = set.get("rest-break").unwrap();
        assert_eq!(
            rest.trigger,
            Trigger::Interval {
                every: Duration::from_secs(50 * 60),
                break_for: Duration::from_secs(600),
            }
        );
        assert_eq!(rest.escalation.len(), 2);
        assert_eq!(rest.snooze().max_per_day, 3);
        assert_eq!(rest.level_at(Duration::from_secs(10)), NagLevel::Toast);
        assert_eq!(rest.level_at(Duration::from_secs(90)), NagLevel::Overlay);

        let micro = set.get("micro-break").unwrap();
        assert_eq!(micro.scope, Scope::Any);
        assert_eq!(micro.idle_policy(), IdlePolicy::Pause);
        assert!(micro.enabled);

        let gaming = set.get("gaming-daily").unwrap();
        assert_eq!(gaming.scope, Scope::Category(CategoryKind::Game));
        assert_eq!(gaming.idle_policy(), IdlePolicy::Ignore);

        let late = set.get("no-late-gaming").unwrap();
        let Trigger::Schedule { windows } = &late.trigger else {
            panic!("expected schedule");
        };
        assert!(windows[0].contains("23:30".parse().unwrap()));
        assert!(windows[0].contains("05:59".parse().unwrap()));
        assert!(!windows[0].contains("06:00".parse().unwrap()));
        assert!(!windows[0].contains("12:00".parse().unwrap()));
        assert!(late.message().contains("allowed hours"));
    }

    #[test]
    fn round_trips_through_toml() {
        let set = RuleSet::from_toml(DEFAULT_CONFIG_TOML).unwrap();
        let text = toml::to_string(&set).unwrap();
        assert_eq!(RuleSet::from_toml(&text).unwrap(), set);
    }

    #[test]
    fn rejects_bad_rules() {
        let dup = r#"
            [[rules]]
            id = "a"
            trigger = { kind = "interval", every = "1m", break_for = "1s" }
            [[rules]]
            id = "a"
            trigger = { kind = "interval", every = "1m", break_for = "1s" }
        "#;
        assert!(matches!(
            RuleSet::from_toml(dup),
            Err(RuleError::DuplicateId(_))
        ));
        let zero = r#"
            [[rules]]
            id = "a"
            trigger = { kind = "interval", every = "0s", break_for = "1s" }
        "#;
        assert!(matches!(
            RuleSet::from_toml(zero),
            Err(RuleError::Invalid { .. })
        ));
        let scope = r#"
            [[rules]]
            id = "a"
            scope = "everything"
            trigger = { kind = "interval", every = "1m", break_for = "1s" }
        "#;
        assert!(RuleSet::from_toml(scope).is_err());
        let app = r#"
            [[rules]]
            id = "a"
            scope = { app = "game.exe" }
            trigger = { kind = "continuous", limit = "1h", reset_after = "10m" }
        "#;
        let set = RuleSet::from_toml(app).unwrap();
        assert_eq!(set.rules[0].scope, Scope::App("game.exe".into()));
    }
}
