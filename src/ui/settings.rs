//! Settings UI components.

#[path = "settings/page.rs"]
mod page;
#[path = "settings/shell_final.rs"]
mod shell_final;
#[path = "settings/stream_sources.rs"]
mod stream_sources;

pub(crate) use shell_final::SettingsPage;
