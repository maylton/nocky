//! Nocky application lifecycle.

mod application;
pub(crate) mod controller;
pub(crate) mod library_state;
pub(crate) mod media;
pub(crate) mod sidebar;
pub(crate) mod state;

pub use application::run;
