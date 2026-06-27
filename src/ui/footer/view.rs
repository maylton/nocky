//! Final visual assembly of the footer.
//!
//! This module composes the existing footer subcomponents. Playback callbacks,
//! MPRIS synchronization, compact reveal callbacks and application state remain
//! owned by `AppController`.

use super::{
    now_playing::{build_footer_now_playing, FooterNowPlayingParts},
    progress::{build_footer_progress, FooterProgressParts},
    transport::{build_footer_transport, FooterTransportParts},
    utilities::{build_footer_utilities, FooterUtilityParts},
};
use crate::{
    config::AppLanguage,
    ui::widgets::{ExpressiveTransport, WaveProgress},
};
use gtk::prelude::*;
use std::rc::Rc;

// nocky_rust_ui_phase3g_footer_view_assembly_v1

const ROOT_HEIGHT: i32 = 88;
const ROOT_CLASSES: [&str; 3] = ["player-bar", "player-bar-v2", "expressive-footer"];

pub(crate) struct FooterViewParts {
    pub(crate) root: gtk::CenterBox,

    pub(crate) now_playing_button: gtk::Button,
    pub(crate) title: gtk::Label,
    pub(crate) artist: gtk::Label,
    pub(crate) source: gtk::Label,
    pub(crate) favorite_button: gtk::Button,
    pub(crate) favorite_icon: gtk::Image,

    pub(crate) center: gtk::Box,
    pub(crate) progress_stack: gtk::Stack,
    pub(crate) traditional_progress: gtk::Scale,
    pub(crate) wave_progress: WaveProgress,
    pub(crate) elapsed: gtk::Label,
    pub(crate) duration: gtk::Label,

    pub(crate) previous: gtk::Button,
    pub(crate) play_button: gtk::Button,
    pub(crate) play_icon: gtk::Image,
    pub(crate) transport_motion: Rc<ExpressiveTransport>,
    pub(crate) next: gtk::Button,
    pub(crate) repeat: gtk::ToggleButton,
    pub(crate) shuffle: gtk::ToggleButton,

    pub(crate) right_controls: gtk::Box,
    pub(crate) lyrics_button: gtk::ToggleButton,
    pub(crate) mute_icon: gtk::Image,
    pub(crate) mute_button: gtk::Button,
    pub(crate) volume: gtk::Adjustment,
    pub(crate) volume_revealer: gtk::Revealer,
}

pub(crate) fn build_footer_view(
    language: AppLanguage,
    initial_volume: f64,
    expressive_effects_enabled: bool,
    artwork: &gtk::Stack,
) -> FooterViewParts {
    let FooterNowPlayingParts {
        button: now_playing_button,
        title,
        artist,
        source,
        favorite_button,
        favorite_icon,
    } = build_footer_now_playing(language, artwork);

    let FooterTransportParts {
        root: transport,
        shuffle,
        previous,
        play_button,
        play_icon,
        motion: transport_motion,
        next,
        repeat,
    } = build_footer_transport(language, expressive_effects_enabled);

    let FooterProgressParts {
        root: center,
        stack: progress_stack,
        traditional: traditional_progress,
        wave: wave_progress,
        elapsed,
        duration,
    } = build_footer_progress(&transport);

    let FooterUtilityParts {
        root: right_controls,
        lyrics_button,
        mute_icon,
        mute_button,
        volume,
        volume_revealer,
    } = build_footer_utilities(language, initial_volume);

    let root = gtk::CenterBox::new();
    root.set_height_request(ROOT_HEIGHT);
    for class in ROOT_CLASSES {
        root.add_css_class(class);
    }
    root.set_start_widget(Some(&now_playing_button));
    root.set_center_widget(Some(&center));
    root.set_end_widget(Some(&right_controls));

    FooterViewParts {
        root,
        now_playing_button,
        title,
        artist,
        source,
        favorite_button,
        favorite_icon,
        center,
        progress_stack,
        traditional_progress,
        wave_progress,
        elapsed,
        duration,
        previous,
        play_button,
        play_icon,
        transport_motion,
        next,
        repeat,
        shuffle,
        right_controls,
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
    fn root_geometry_matches_the_approved_footer() {
        assert_eq!(ROOT_HEIGHT, 88);
    }

    #[test]
    fn root_css_contract_remains_stable() {
        assert_eq!(
            ROOT_CLASSES,
            ["player-bar", "player-bar-v2", "expressive-footer",]
        );
    }
}
