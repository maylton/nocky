//! Playback domain.
//!
//! This module groups the audio engine, playback persistence, MPRIS,
//! transitions, and queue infrastructure without changing runtime behavior.

pub mod engine;
pub mod mpris;
pub mod queue;
pub mod session;
pub mod transition;

pub use engine::*;
