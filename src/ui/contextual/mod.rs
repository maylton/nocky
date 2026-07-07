//! Reusable contextual surfaces.
//!
//! Keep action/menu construction out of feature controllers so Queue, Home,
//! playlists, albums, artists, and dialogs can share the same Material
//! Expressive menu language.

mod action;
mod menu;

pub(crate) use action::{build_contextual_action, CONTEXTUAL_MENU_ACTION_CLASS};
pub(crate) use menu::{MaterialContextMenu, CONTEXTUAL_MENU_CLASS};
