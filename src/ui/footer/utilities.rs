//! Visual construction of the footer utility and volume controls.
//!
//! Mute state, compact expansion, reveal callbacks and spring motion remain
//! owned by `AppController`.

use crate::{
    config::AppLanguage,
    i18n::{self, Message},
    md3_volume::Md3VolumeSlider,
    ui::widgets::material_button::{
        apply_material_icon_button, MaterialIconButtonSpec, MaterialIconButtonVariant,
    },
};
use gtk::prelude::*;
const VOLUME_STEP: f64 = 0.01;
const VOLUME_PAGE_STEP: f64 = 0.05;
const SLOT_WIDTH: i32 = 124;
const SLOT_HEIGHT: i32 = 42;
const CANVAS_WIDTH: i32 = 116;
const CANVAS_HEIGHT: i32 = 42;
const CANVAS_X: f64 = 4.0;
const REVEAL_DURATION_MS: u32 = 280;
const GROUP_SPACING: i32 = 6;
const GROUP_MARGIN_TOP: i32 = 0;
const GROUP_WIDTH: i32 = 220;
const GROUP_HEIGHT: i32 = 56;

pub(crate) struct FooterUtilityParts {
    pub(crate) root: gtk::Box,
    pub(crate) lyrics_button: gtk::ToggleButton,
    pub(crate) mute_icon: gtk::Image,
    pub(crate) mute_button: gtk::Button,
    pub(crate) volume: gtk::Adjustment,
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
    apply_material_icon_button(
        &lyrics_button,
        MaterialIconButtonSpec::new(MaterialIconButtonVariant::Standard),
    );
    lyrics_button.add_css_class("footer-control");
    lyrics_button.add_css_class("footer-lyrics-button");
    lyrics_button.add_css_class("footer-utility-action");
    lyrics_button.set_valign(gtk::Align::Center);

    let mute_icon = gtk::Image::from_icon_name("audio-volume-high-symbolic");
    let mute_button = gtk::Button::new();
    mute_button.set_child(Some(&mute_icon));
    apply_material_icon_button(
        &mute_button,
        MaterialIconButtonSpec::new(MaterialIconButtonVariant::Standard),
    );
    mute_button.add_css_class("footer-control");
    mute_button.add_css_class("footer-utility-action");
    mute_button.set_valign(gtk::Align::Center);
    mute_button.set_tooltip_text(Some(tr(Message::Mute)));
    // The visible control is custom-drawn. Use a non-widget Adjustment as its
    // shared value model so GTK never measures an invisible Scale gadget.
    let volume = gtk::Adjustment::new(
        initial_volume.clamp(0.0, 1.0),
        0.0,
        1.0,
        VOLUME_STEP,
        VOLUME_PAGE_STEP,
        0.0,
    );
    let volume_slot = gtk::Fixed::new();
    volume_slot.set_size_request(SLOT_WIDTH, SLOT_HEIGHT);
    volume_slot.set_hexpand(false);
    volume_slot.set_vexpand(false);
    volume_slot.set_overflow(gtk::Overflow::Hidden);
    volume_slot.add_css_class("footer-volume-fixed-slot");
    let md3_volume = Md3VolumeSlider::new(&volume);
    md3_volume
        .widget()
        .set_size_request(CANVAS_WIDTH, CANVAS_HEIGHT);
    volume_slot.put(md3_volume.widget(), CANVAS_X, 0.0);
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
    fn volume_adjustment_contract_remains_stable() {
        assert_eq!(VOLUME_STEP, 0.01);
        assert_eq!(VOLUME_PAGE_STEP, 0.05);
    }
}
