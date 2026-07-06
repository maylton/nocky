//! Nocky Connect desktop integration foundation.
//!
//! This module is intentionally isolated from the UI and playback engine for now.
//! It defines the portable snapshot format shared with Android and provides
//! export/restore helpers for the existing desktop queue model.

pub mod device_identity;
pub mod file_store;
pub mod gateway;
pub mod mapper;
pub mod protocol;

pub use device_identity::*;
pub use file_store::*;
pub use gateway::*;
pub use mapper::*;
pub use protocol::*;
