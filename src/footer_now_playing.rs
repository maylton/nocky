//! Visual construction of the footer now-playing component.
//!
//! Playback callbacks, metadata updates and footer layout policy remain owned
//! by `AppController`.

use crate::{
    config::AppLanguage,
    i18n::{self, Message},
};
use gtk::prelude::*;

// nocky_rust_ui_phase3c_footer_now_playing_v1

// nocky_footer_optical_alignment_metadata_width_v1

const TITLE_MAX_WIDTH_CHARS: i32 = 22;
const ARTIST_MAX_WIDTH_CHARS: i32 = 18;

pub(crate) struct FooterNowPlayingParts {
    pub(crate) button: gtk::Button,
    pub(crate) title: gtk::Label,
    pub(crate) artist: gtk::Label,
    pub(crate) source: gtk::Label,
    pub(crate) favorite_button: gtk::Button,
    pub(crate) favorite_icon: gtk::Image,
}

pub(crate) fn build_footer_now_playing(
    language: AppLanguage,
    artwork: &gtk::Stack,
) -> FooterNowPlayingParts {
    let tr = |message| i18n::text(language, message);

    artwork.add_css_class("footer-artwork");

    let title = gtk::Label::new(Some(tr(Message::NothingPlaying)));
    title.set_xalign(0.0);
    title.set_ellipsize(gtk::pango::EllipsizeMode::End);
    title.add_css_class("now-title");
    title.add_css_class("footer-track-title");
    title.set_hexpand(true);
    title.set_width_chars(-1);
    title.set_max_width_chars(TITLE_MAX_WIDTH_CHARS);

    let favorite_icon = gtk::Image::from_icon_name("emblem-favorite-symbolic");
    favorite_icon.set_opacity(0.28);

    let favorite_button = gtk::Button::new();
    favorite_button.set_child(Some(&favorite_icon));
    favorite_button.add_css_class("flat");
    favorite_button.add_css_class("footer-control");
    favorite_button.add_css_class("footer-favorite-button");
    favorite_button.add_css_class("footer-favorite-action");
    favorite_button.set_tooltip_text(Some(tr(Message::FavoriteTooltip)));

    let artist = gtk::Label::new(Some("Nocky"));
    artist.set_xalign(0.0);
    artist.set_ellipsize(gtk::pango::EllipsizeMode::End);
    artist.add_css_class("dim-label");
    artist.add_css_class("footer-track-artist");
    artist.set_hexpand(false);
    artist.set_width_chars(-1);
    artist.set_max_width_chars(ARTIST_MAX_WIDTH_CHARS);

    let source = gtk::Label::new(Some(tr(Message::SourceNone)));
    source.add_css_class("source-badge");
    source.add_css_class("footer-source-badge");
    source.add_css_class("footer-source-pill");
    source.set_valign(gtk::Align::Center);

    let title_row = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    title_row.set_halign(gtk::Align::Start);
    title_row.add_css_class("footer-title-row");
    title_row.append(&title);

    let artist_row = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    artist_row.set_halign(gtk::Align::Start);
    artist_row.add_css_class("footer-artist-row");
    artist_row.append(&artist);

    let action_row = gtk::Box::new(gtk::Orientation::Horizontal, 5);
    action_row.set_halign(gtk::Align::Start);
    action_row.set_valign(gtk::Align::Center);
    action_row.add_css_class("footer-action-row");
    action_row.append(&favorite_button);
    action_row.append(&source);

    let metadata = gtk::Box::new(gtk::Orientation::Vertical, 0);
    metadata.set_hexpand(false);
    metadata.set_halign(gtk::Align::Start);
    metadata.set_valign(gtk::Align::Center);
    metadata.add_css_class("footer-meta");
    metadata.add_css_class("footer-metadata");
    metadata.append(&title_row);
    metadata.append(&artist_row);
    metadata.append(&action_row);

    let content = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    content.set_halign(gtk::Align::Start);
    content.set_valign(gtk::Align::Center);
    content.add_css_class("footer-track-content");
    content.append(artwork);
    content.append(&metadata);

    let button = gtk::Button::new();
    button.set_child(Some(&content));
    button.set_size_request(350, 56);
    button.set_hexpand(false);
    button.set_halign(gtk::Align::Start);
    button.add_css_class("flat");
    button.add_css_class("footer-now-playing-button");
    button.add_css_class("footer-info-card");
    button.set_tooltip_text(Some("Abrir fila de reprodução"));
    button.set_valign(gtk::Align::Center);

    FooterNowPlayingParts {
        button,
        title,
        artist,
        source,
        favorite_button,
        favorite_icon,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_width_contract_is_stable() {
        assert_eq!(TITLE_MAX_WIDTH_CHARS, 22);
        assert_eq!(ARTIST_MAX_WIDTH_CHARS, 18);
    }

    #[test]
    fn translated_copy_exists_for_every_supported_language() {
        for language in [
            AppLanguage::Portuguese,
            AppLanguage::English,
            AppLanguage::Spanish,
        ] {
            assert!(!i18n::text(language, Message::NothingPlaying).is_empty());
            assert!(!i18n::text(language, Message::FavoriteTooltip).is_empty());
            assert!(!i18n::text(language, Message::SourceNone).is_empty());
        }
    }
}
