//! Rules and the nag engine.
//!
//! [`RuleSet`] parses the `[[rules]]` tables of the config file. [`Engine`]
//! is pure logic driven by ticks, the current [`an_core::Activity`] and
//! [`an_core::ControlCommand`]s; it emits [`an_core::NagEvent`]s.
//! [`EngineTask`] runs it on the async runtime.
//!
//! Lock and idle time count as rest (Workrave semantics). Reading mode turns
//! every rule's idle policy into `ignore`.

pub mod engine;
pub mod rule;
pub mod task;

pub use an_core::{ControlCommand, NagEvent, NagLevel};
pub use engine::{Engine, EngineSnapshot, NagState, RuleSnapshot};
pub use rule::{EscalationStep, IdlePolicy, Rule, RuleError, RuleSet, Scope, Snooze, Trigger};
pub use task::{EngineCommand, EngineTask};
