//! Shared UI widgets.

mod animated_page_switcher;
mod compact_volume_motion;
mod cover;
mod expressive_loading;
mod expressive_transport;
// The foundation intentionally exposes variants before call sites migrate.
// Remove this allowance when the Settings pilot buttons consume the helper.
#[allow(dead_code)]
pub(crate) mod material_button;
mod wave_progress;

pub(crate) use animated_page_switcher::{AnimatedPageSpec, AnimatedPageSwitcher, TopPage};
pub(crate) use compact_volume_motion::{run_compact_volume_spring, CompactVolumeSpring};
pub(crate) use cover::{build_cover, CoverView};
pub(crate) use expressive_loading::MaterialLoadingIndicator;
#[cfg(feature = "assisted-login")]
pub(crate) use expressive_loading::{
    LoadingIndicatorMode, LoadingIndicatorPresentation, LoadingIndicatorSize,
};
pub(crate) use expressive_transport::{ExpressiveTransport, TransportVariant};
pub(crate) use wave_progress::WaveProgress;
