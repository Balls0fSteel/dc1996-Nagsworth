//! Turns platform samples into stored segments.
//!
//! [`Sampler`] polls the platform once per interval; [`segmenter`] opens and
//! closes segments on (app, title, kind) changes and publishes the current
//! [`an_core::Activity`]; [`Categorizer`] assigns new apps a category;
//! [`web`] turns browser-extension tab events into web visits.

pub mod categorize;
pub mod gaming;
pub mod sampler;
pub mod segmenter;
pub mod web;

pub use categorize::Categorizer;
pub use sampler::Sampler;
pub use segmenter::{Segmenter, SegmenterConfig};
pub use web::{ExtEvent, ExtEventKind, WebSegmenter};
