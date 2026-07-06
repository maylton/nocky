//! Shared footer UI components.

mod connect;
mod layout;
mod now_playing;
mod progress;
mod transport;
mod utilities;
mod view;

pub(crate) use layout::{
    footer_full_artwork_size_for_card_height, footer_mode_plan, AdaptiveFooterTier,
    FOOTER_ARTWORK_SOURCE_SIZE, FOOTER_COMPACT_CARD_MARGIN,
};
pub(crate) use view::{build_footer_view, FooterViewParts};
