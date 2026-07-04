//! Action contract for Home V3 feed items.
//!
//! MetroList-style Home items should preserve their endpoint behavior: playable
//! items play, browse endpoints navigate, and incomplete items do nothing.

#[cfg(test)]
use crate::home_v3::HomeV3Item;
#[cfg(not(test))]
use super::home_v3::HomeV3Item;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum HomeV3ItemAction {
    Play { video_id: String },
    Browse { browse_id: String, params: String },
    None,
}

pub(crate) fn item_action(item: &HomeV3Item) -> HomeV3ItemAction {
    let video_id = item.video_id.trim();
    if !video_id.is_empty() {
        return HomeV3ItemAction::Play {
            video_id: video_id.to_string(),
        };
    }

    let browse_id = item.browse_id.trim();
    if !browse_id.is_empty() {
        return HomeV3ItemAction::Browse {
            browse_id: browse_id.to_string(),
            params: item.params.trim().to_string(),
        };
    }

    HomeV3ItemAction::None
}
