//! Nocky Connect desktop integration foundation.
//!
//! This module is intentionally isolated from the UI and playback engine for now.
//! It defines the portable snapshot format shared with Android and provides
//! export/restore helpers for the existing desktop queue model.

pub mod desktop_services;
pub mod device_descriptor;
pub mod device_identity;
pub mod device_list;
pub mod discovery;
pub mod discovery_udp;
pub mod file_store;
pub mod gateway;
pub mod handoff_http;
pub mod handoff_http_receiver;
pub mod handoff_target;
pub mod mapper;
pub mod protocol;

pub use desktop_services::*;
pub use device_descriptor::*;
pub use device_identity::*;
pub use device_list::*;
pub use discovery::*;
pub use discovery_udp::*;
pub use file_store::*;
pub use gateway::*;
pub use handoff_http::*;
pub use handoff_http_receiver::*;
pub use handoff_target::*;
pub use mapper::*;
pub use protocol::*;
