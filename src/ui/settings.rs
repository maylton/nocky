//! Settings UI components.

#[path = "settings/page.rs"]
mod page;
#[path = "settings/shell_clean.rs"]
mod shell_clean;
#[path = "settings/stream_sources.rs"]
mod stream_sources;

pub(crate) use shell_clean::SettingsPage;
