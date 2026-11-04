//! Rules and the nag engine.
//!
//! [`RuleSet`] parses the `[[rules]]` tables of the config file. [`Engine`]
//! is pure logic driven by ticks, the current [`ns_core::Activity`] and
//! [`ns_core::ControlCommand`]s; it emits [`ns_core::NagEvent`]s.
//! [`EngineTask`] runs it on the async runtime.
//!
//! Lock and idle time count as rest (Workrave semantics). Reading mode turns
//! every rule's idle policy into `ignore`.

pub mod engine;
pub mod rule;
pub mod task;

pub use engine::{Engine, EngineSnapshot, NagState, RuleSnapshot};
pub use ns_core::{ControlCommand, NagEvent, NagLevel};
pub use rule::{EscalationStep, IdlePolicy, Rule, RuleError, RuleSet, Scope, Snooze, Trigger};
pub use task::{EngineCommand, EngineTask};
