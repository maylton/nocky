//! Visual construction of the footer utility and volume controls.
//!
//! Mute state, compact expansion, reveal callbacks and spring motion remain
//! owned by `AppController`.

use crate::{
    config::AppLanguage,
    i18n::{self, Message},
    md3_volume::Md3VolumeSlider,
};
use gtk::prelude::*;

// nocky_rust_ui_phase3f_footer_utilities_v1

const VOLUME_STEP: f64 = 0.01;
const INITIAL_SCALE_WIDTH: i32 = 96;
const SLOT_WIDTH: i32 = 124;
const SLOT_HEIGHT: i32 = 42;
const CANVAS_WIDTH: i32 = 116;
const CANVAS_HEIGHT: i32 = 42;
const CANVAS_X: f64 = 4.0;
const REVEAL_DURATION_MS: u32 = 280;
const GROUP_SPACING: i32 = 6;
// nocky_footer_optical_alignment_metadata_width_v1
const GROUP_MARGIN_TOP: i32 = 0;
const GROUP_WIDTH: i32 = 220;
const GROUP_HEIGHT: i32 = 56;

pub(crate) struct FooterUtilityParts {
    pub(crate) root: gtk::Box,
    pub(crate) lyrics_button: gtk::ToggleButton,
    pub(crate) mute_icon: gtk::Image,
    pub(crate) mute_button: gtk::Button,
    pub(crate) volume: gtk::Scale,
    pub(crate) volume_revealer: gtk::Revealer,
}

pub(crate) fn build_footer_utilities(
    language: AppLanguage,
    initial_volume: f64,
) -> FooterUtilityParts {
    let tr = |message| i18n::text(language, message);

    let lyrics_button = gtk::ToggleButton::builder()
        .icon_name("audio-input-microphone-symbolic")
        .tooltip_text(tr(Message::LyricsTooltip))
        .build();
    lyrics_button.add_css_class("flat");
    lyrics_button.add_css_class("footer-control");
    lyrics_button.add_css_class("footer-lyrics-button");
    lyrics_button.add_css_class("footer-utility-action");
    lyrics_button.set_valign(gtk::Align::Center);

    let mute_icon = gtk::Image::from_icon_name("audio-volume-high-symbolic");
    let mute_button = gtk::Button::new();
    mute_button.set_child(Some(&mute_icon));
    mute_button.add_css_class("flat");
    mute_button.add_css_class("footer-control");
    mute_button.add_css_class("footer-utility-action");
    mute_button.set_valign(gtk::Align::Center);
    mute_button.set_tooltip_text(Some(tr(Message::Mute)));

    let volume = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, 1.0, VOLUME_STEP);
    volume.set_draw_value(false);
    volume.set_value(initial_volume.clamp(0.0, 1.0));
    volume.set_size_request(INITIAL_SCALE_WIDTH, -1);
    volume.set_valign(gtk::Align::Center);
    volume.add_css_class("footer-volume");
    volume.add_css_class("footer-volume-control");

    // nocky_compact_volume_expand_and_flat_modes_v1
    let volume_slot = gtk::Fixed::new();
    volume_slot.set_size_request(SLOT_WIDTH, SLOT_HEIGHT);
    volume_slot.set_hexpand(false);
    volume_slot.set_vexpand(false);
    volume_slot.set_overflow(gtk::Overflow::Hidden);
    volume_slot.add_css_class("footer-volume-fixed-slot");

    // nocky_compact_volume_fixed_slot_reveal_v1
    volume.set_size_request(CANVAS_WIDTH, CANVAS_HEIGHT);
    volume.set_has_origin(true);
    volume.add_css_class("footer-volume-md3");

    let md3_volume = Md3VolumeSlider::new(&volume);
    volume.set_visible(false);
    volume_slot.put(md3_volume.widget(), CANVAS_X, 0.0);

    // nocky_md3_volume_slider_right_v1
    let volume_revealer = gtk::Revealer::new();
    volume_revealer.set_transition_type(gtk::RevealerTransitionType::SlideRight);
    volume_revealer.set_transition_duration(REVEAL_DURATION_MS);
    volume_revealer.set_reveal_child(true);
    volume_revealer.set_halign(gtk::Align::Start);
    volume_revealer.set_valign(gtk::Align::Center);
    volume_revealer.set_child(Some(&volume_slot));
    volume_revealer.add_css_class("footer-volume-revealer");

    let root = gtk::Box::new(gtk::Orientation::Horizontal, GROUP_SPACING);
    root.set_margin_top(GROUP_MARGIN_TOP);
    root.set_halign(gtk::Align::End);
    root.set_valign(gtk::Align::Center);
    root.add_css_class("footer-utility-group");
    root.set_size_request(GROUP_WIDTH, GROUP_HEIGHT);
    root.append(&lyrics_button);
    root.append(&mute_button);
    root.append(&volume_revealer);

    FooterUtilityParts {
        root,
        lyrics_button,
        mute_icon,
        mute_button,
        volume,
        volume_revealer,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utility_copy_exists_for_every_supported_language() {
        for language in [
            AppLanguage::Portuguese,
            AppLanguage::English,
            AppLanguage::Spanish,
        ] {
            assert!(!i18n::text(language, Message::LyricsTooltip).is_empty());
            assert!(!i18n::text(language, Message::Mute).is_empty());
        }
    }

    #[test]
    fn compact_volume_geometry_matches_the_approved_design() {
        assert_eq!((SLOT_WIDTH, SLOT_HEIGHT), (124, 42));
        assert_eq!((CANVAS_WIDTH, CANVAS_HEIGHT), (116, 42));
        assert_eq!(CANVAS_X, 4.0);
        assert_eq!(REVEAL_DURATION_MS, 280);
        assert_eq!((GROUP_WIDTH, GROUP_HEIGHT), (220, 56));
        assert_eq!((GROUP_SPACING, GROUP_MARGIN_TOP), (6, 0));
    }

    #[test]
    fn volume_step_and_initial_width_remain_stable() {
        assert_eq!(VOLUME_STEP, 0.01);
        assert_eq!(INITIAL_SCALE_WIDTH, 96);
    }
}
