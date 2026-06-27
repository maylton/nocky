//! Pure footer mode and responsive-layout policy.
//!
//! GTK widget mutation remains in `AppController`; this module freezes the
//! approved mode resolution, geometry and responsive breakpoints.

use crate::config::FooterMode;

// nocky_rust_ui_phase3b_footer_layout_policy_v1

// nocky_footer_metadata_full_mode_breathing_room_v4
// nocky_footer_full_metadata_visual_density_v7
// nocky_footer_metadata_fill_available_height_v8
// nocky_footer_artwork_tracks_card_height_v11
pub(crate) const FOOTER_FULL_ARTWORK_SIZE: i32 = 72;
pub(crate) const FOOTER_ARTWORK_SOURCE_SIZE: i32 = 96;
const FOOTER_ARTWORK_VERTICAL_PADDING: i32 = 6;
const FOOTER_COMPACT_ARTWORK_SIZE: i32 = 50;
// nocky_footer_compact_restores_vertical_air_v12
pub(crate) const FOOTER_COMPACT_CARD_MARGIN: i32 = 4;
const FULL_METADATA_SPACING: i32 = 0;
const COMPACT_METADATA_SPACING: i32 = 0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct FooterModePlan {
    pub(crate) bar_visible: bool,
    pub(crate) full: bool,
    pub(crate) css_class: &'static str,
    pub(crate) bar_height: i32,
    pub(crate) now_playing_size: (i32, i32),
    pub(crate) now_playing_artwork_size: i32,
    pub(crate) metadata_spacing: i32,
    pub(crate) center_size: (i32, i32),
    pub(crate) right_size: Option<(i32, i32)>,
}

pub(crate) fn footer_full_artwork_size_for_card_height(card_height: i32) -> i32 {
    (card_height - FOOTER_ARTWORK_VERTICAL_PADDING).max(FOOTER_FULL_ARTWORK_SIZE)
}

pub(crate) fn footer_mode_plan(
    configured: FooterMode,
    home_player_visible: bool,
) -> FooterModePlan {
    match resolve_footer_mode(configured, home_player_visible) {
        FooterMode::Hidden => FooterModePlan {
            bar_visible: false,
            full: false,
            css_class: "footer-mode-hidden",
            bar_height: 0,
            now_playing_size: (0, 0),
            now_playing_artwork_size: 0,
            metadata_spacing: 0,
            center_size: (0, 0),
            right_size: None,
        },
        FooterMode::Full => FooterModePlan {
            bar_visible: true,
            full: true,
            css_class: "footer-mode-full",
            bar_height: 86,
            now_playing_size: (330, 72),
            now_playing_artwork_size: FOOTER_FULL_ARTWORK_SIZE,
            metadata_spacing: FULL_METADATA_SPACING,
            center_size: (470, 56),
            right_size: Some((190, 52)),
        },
        FooterMode::Compact | FooterMode::Automatic => FooterModePlan {
            bar_visible: true,
            full: false,
            css_class: "footer-mode-compact",
            bar_height: 70,
            now_playing_size: (292, 52),
            now_playing_artwork_size: FOOTER_COMPACT_ARTWORK_SIZE,
            metadata_spacing: COMPACT_METADATA_SPACING,
            center_size: (0, 52),
            right_size: None,
        },
    }
}

fn resolve_footer_mode(configured: FooterMode, home_player_visible: bool) -> FooterMode {
    match configured {
        FooterMode::Automatic if home_player_visible => FooterMode::Compact,
        FooterMode::Automatic => FooterMode::Full,
        other => other,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AdaptiveFooterTier {
    Wide,
    Medium,
    Narrow,
}

impl AdaptiveFooterTier {
    pub(crate) fn for_width(width: i32) -> Self {
        if width >= 1040 {
            Self::Wide
        } else if width >= 790 {
            Self::Medium
        } else {
            Self::Narrow
        }
    }

    pub(crate) fn plan(self) -> AdaptiveFooterPlan {
        match self {
            Self::Wide => AdaptiveFooterPlan {
                now_playing_size: (350, 72),
                center_size: (500, 60),
                right_size: (220, 56),
                show_source: true,
                show_artist: true,
                show_elapsed: true,
                show_duration: true,
                show_shuffle: true,
                show_repeat: true,
                show_volume: true,
            },
            Self::Medium => AdaptiveFooterPlan {
                now_playing_size: (280, 72),
                center_size: (390, 60),
                right_size: (98, 56),
                show_source: false,
                show_artist: true,
                show_elapsed: false,
                show_duration: false,
                show_shuffle: true,
                show_repeat: true,
                show_volume: false,
            },
            Self::Narrow => AdaptiveFooterPlan {
                now_playing_size: (190, 72),
                center_size: (190, 60),
                right_size: (92, 56),
                show_source: false,
                show_artist: false,
                show_elapsed: false,
                show_duration: false,
                show_shuffle: false,
                show_repeat: false,
                show_volume: false,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct AdaptiveFooterPlan {
    pub(crate) now_playing_size: (i32, i32),
    pub(crate) center_size: (i32, i32),
    pub(crate) right_size: (i32, i32),
    pub(crate) show_source: bool,
    pub(crate) show_artist: bool,
    pub(crate) show_elapsed: bool,
    pub(crate) show_duration: bool,
    pub(crate) show_shuffle: bool,
    pub(crate) show_repeat: bool,
    pub(crate) show_volume: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compact_card_keeps_vertical_breathing_room() {
        assert_eq!(FOOTER_COMPACT_CARD_MARGIN, 4);
    }

    #[test]
    fn full_artwork_tracks_the_allocated_card_height() {
        assert_eq!(
            footer_full_artwork_size_for_card_height(72),
            FOOTER_FULL_ARTWORK_SIZE
        );
        assert_eq!(footer_full_artwork_size_for_card_height(78), 72);
        assert_eq!(footer_full_artwork_size_for_card_height(84), 78);
        assert_eq!(FOOTER_ARTWORK_SOURCE_SIZE, 96);
    }

    #[test]
    fn automatic_is_compact_while_home_player_is_visible() {
        let plan = footer_mode_plan(FooterMode::Automatic, true);
        assert_eq!(plan.css_class, "footer-mode-compact");
        assert!(!plan.full);
    }

    #[test]
    fn automatic_is_full_outside_the_visible_home_player() {
        let plan = footer_mode_plan(FooterMode::Automatic, false);
        assert_eq!(plan.css_class, "footer-mode-full");
        assert!(plan.full);
    }

    #[test]
    fn explicit_hidden_mode_is_preserved() {
        let plan = footer_mode_plan(FooterMode::Hidden, true);
        assert!(!plan.bar_visible);
        assert_eq!(plan.css_class, "footer-mode-hidden");
    }

    #[test]
    fn full_geometry_matches_the_approved_footer() {
        let plan = footer_mode_plan(FooterMode::Full, true);
        assert_eq!(plan.bar_height, 86);
        assert_eq!(plan.now_playing_size, (330, 72));
        assert_eq!(plan.now_playing_artwork_size, FOOTER_FULL_ARTWORK_SIZE);
        assert_eq!(plan.metadata_spacing, FULL_METADATA_SPACING);
        assert_eq!(plan.center_size, (470, 56));
        assert_eq!(plan.right_size, Some((190, 52)));
    }

    #[test]
    fn compact_geometry_matches_the_approved_footer() {
        let plan = footer_mode_plan(FooterMode::Compact, false);
        assert_eq!(plan.bar_height, 70);
        assert_eq!(plan.now_playing_size, (292, 52));
        assert_eq!(plan.now_playing_artwork_size, FOOTER_COMPACT_ARTWORK_SIZE);
        assert_eq!(plan.metadata_spacing, COMPACT_METADATA_SPACING);
        assert_eq!(plan.center_size, (0, 52));
        assert_eq!(plan.right_size, None);
    }

    #[test]
    fn responsive_breakpoints_keep_the_exact_boundaries() {
        assert_eq!(
            AdaptiveFooterTier::for_width(1040),
            AdaptiveFooterTier::Wide
        );
        assert_eq!(
            AdaptiveFooterTier::for_width(1039),
            AdaptiveFooterTier::Medium
        );
        assert_eq!(
            AdaptiveFooterTier::for_width(790),
            AdaptiveFooterTier::Medium
        );
        assert_eq!(
            AdaptiveFooterTier::for_width(789),
            AdaptiveFooterTier::Narrow
        );
    }

    #[test]
    fn medium_layout_hides_only_the_expected_controls() {
        let plan = AdaptiveFooterTier::Medium.plan();
        assert_eq!(plan.now_playing_size, (280, 72));
        assert_eq!(plan.center_size, (390, 60));
        assert_eq!(plan.right_size, (98, 56));
        assert!(!plan.show_source);
        assert!(plan.show_artist);
        assert!(!plan.show_elapsed);
        assert!(plan.show_shuffle);
        assert!(!plan.show_volume);
    }

    #[test]
    fn narrow_layout_keeps_only_the_core_footer_content() {
        let plan = AdaptiveFooterTier::Narrow.plan();
        assert_eq!(plan.now_playing_size, (190, 72));
        assert_eq!(plan.center_size, (190, 60));
        assert_eq!(plan.right_size, (92, 56));
        assert!(!plan.show_artist);
        assert!(!plan.show_shuffle);
        assert!(!plan.show_repeat);
        assert!(!plan.show_volume);
    }
}
