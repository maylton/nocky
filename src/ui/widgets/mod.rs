//! Shared UI widgets.

mod animated_page_switcher;
mod artwork_cache;
mod compact_volume_motion;
mod cover;
mod expressive_loading;
mod expressive_transport;
mod wave_progress;

pub(crate) use animated_page_switcher::{AnimatedPageSpec, AnimatedPageSwitcher, TopPage};
pub(crate) use compact_volume_motion::{run_compact_volume_spring, CompactVolumeSpring};
pub(crate) use cover::{build_cover, CoverView};
pub(crate) use expressive_loading::ExpressiveLoadingIndicator;
pub(crate) use expressive_transport::{ExpressiveTransport, TransportVariant};
pub(crate) use wave_progress::WaveProgress;
