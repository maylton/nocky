//! Visual construction of the complete-footer transport controls.
//!
//! Playback callbacks, state synchronization, mode application and progress
//! remain owned by `AppController`.

use crate::{
    config::AppLanguage,
    expressive_transport::{ExpressiveTransport, TransportVariant},
    i18n::{self, Message},
    mode_toggle,
};
use gtk::prelude::*;
use std::rc::Rc;

// nocky_rust_ui_phase3d_footer_transport_v2

pub(crate) struct FooterTransportParts {
    pub(crate) root: gtk::Box,
    pub(crate) shuffle: gtk::ToggleButton,
    pub(crate) previous: gtk::Button,
    pub(crate) play_button: gtk::Button,
    pub(crate) play_icon: gtk::Image,
    pub(crate) motion: Rc<ExpressiveTransport>,
    pub(crate) next: gtk::Button,
    pub(crate) repeat: gtk::ToggleButton,
}

pub(crate) fn build_footer_transport(
    language: AppLanguage,
    expressive_effects_enabled: bool,
) -> FooterTransportParts {
    let tr = |message| i18n::text(language, message);

    let shuffle = mode_toggle::new_mode_toggle(
        "media-playlist-shuffle-symbolic",
        tr(Message::Shuffle),
        mode_toggle::ModeToggleKind::Shuffle,
    );
    shuffle.add_css_class("flat");
    shuffle.add_css_class("footer-control");
    shuffle.add_css_class("footer-mode-control");

    let previous = gtk::Button::from_icon_name("media-skip-backward-symbolic");
    previous.set_tooltip_text(Some(tr(Message::PreviousTrack)));
    previous.add_css_class("flat");
    previous.add_css_class("footer-control");
    previous.add_css_class("footer-skip-control");

    let play_icon = gtk::Image::from_icon_name("media-playback-start-symbolic");
    play_icon.set_pixel_size(20);

    let play_button = gtk::Button::new();
    play_button.set_child(Some(&play_icon));
    play_button.add_css_class("flat");
    play_button.add_css_class("mini-play-button");
    play_button.add_css_class("footer-primary-control");
    play_button.set_tooltip_text(Some(tr(Message::PlayPause)));

    let next = gtk::Button::from_icon_name("media-skip-forward-symbolic");
    next.set_tooltip_text(Some(tr(Message::NextTrack)));
    next.add_css_class("flat");
    next.add_css_class("footer-control");
    next.add_css_class("footer-skip-control");

    let repeat = mode_toggle::new_mode_toggle(
        "media-playlist-repeat-symbolic",
        tr(Message::RepeatTrack),
        mode_toggle::ModeToggleKind::RepeatOne,
    );
    repeat.add_css_class("flat");
    repeat.add_css_class("footer-control");
    repeat.add_css_class("footer-mode-control");

    let motion = ExpressiveTransport::new(
        TransportVariant::Footer,
        &previous,
        &play_button,
        &next,
        &play_icon,
        expressive_effects_enabled,
    );

    let root = gtk::Box::new(gtk::Orientation::Horizontal, 7);
    root.set_margin_top(0);
    root.set_halign(gtk::Align::Center);
    root.set_valign(gtk::Align::Center);
    root.add_css_class("footer-transport-controls");
    root.append(&shuffle);
    root.append(motion.root());
    root.append(&repeat);

    FooterTransportParts {
        root,
        shuffle,
        previous,
        play_button,
        play_icon,
        motion,
        next,
        repeat,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translated_transport_copy_exists_for_supported_languages() {
        for language in [
            AppLanguage::Portuguese,
            AppLanguage::English,
            AppLanguage::Spanish,
        ] {
            for message in [
                Message::Shuffle,
                Message::PreviousTrack,
                Message::PlayPause,
                Message::NextTrack,
                Message::RepeatTrack,
            ] {
                assert!(!i18n::text(language, message).is_empty());
            }
        }
    }
}
