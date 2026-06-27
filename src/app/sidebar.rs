//! Sidebar construction for the main application shell.

use crate::{
    config::AppLanguage,
    i18n::{self, Message},
};
use gtk::prelude::*;

const SIDEBAR_WIDTH: i32 = 252;

pub(crate) struct SidebarParts {
    pub(crate) revealer: gtk::Revealer,
    pub(crate) motion: gtk::Fixed,
    pub(crate) content: gtk::Box,
    pub(crate) all_button: gtk::Button,
    pub(crate) all_label: gtk::Label,
    pub(crate) albums_button: gtk::Button,
    pub(crate) albums_label: gtk::Label,
    pub(crate) artists_button: gtk::Button,
    pub(crate) artists_label: gtk::Label,
    pub(crate) playlists_button: gtk::Button,
    pub(crate) playlists_label: gtk::Label,
    pub(crate) liked_button: gtk::Button,
    pub(crate) liked_label: gtk::Label,
    pub(crate) section_label: gtk::Label,
}

pub(crate) fn build_sidebar(language: AppLanguage) -> SidebarParts {
    let tr = |message| i18n::text(language, message);
    let content = gtk::Box::new(gtk::Orientation::Vertical, 6);
    content.set_size_request(SIDEBAR_WIDTH, -1);
    content.set_margin_top(12);
    content.set_margin_bottom(12);
    content.set_margin_start(10);
    content.set_margin_end(10);
    content.add_css_class("sidebar-content");

    let (all_button, all_label) = sidebar_row("view-list-symbolic", tr(Message::Library), true);
    let (albums_button, albums_label) =
        sidebar_row("folder-music-symbolic", tr(Message::Albums), false);
    let (artists_button, artists_label) =
        sidebar_row("avatar-default-symbolic", tr(Message::Artists), false);
    let (playlists_button, playlists_label) =
        sidebar_row("view-list-symbolic", tr(Message::Playlists), false);
    content.append(&all_button);
    content.append(&albums_button);
    content.append(&artists_button);
    content.append(&playlists_button);

    let section = gtk::Label::new(Some(tr(Message::LocalCollection)));
    section.set_xalign(0.0);
    section.set_margin_top(18);
    section.set_margin_start(10);
    section.add_css_class("section-title");
    content.append(&section);

    let (liked_button, liked_label) =
        sidebar_row("emblem-favorite-symbolic", tr(Message::LikedSongs), false);
    content.append(&liked_button);

    let spacer = gtk::Box::new(gtk::Orientation::Vertical, 0);
    spacer.set_vexpand(true);
    content.append(&spacer);

    let revealer = gtk::Revealer::new();
    revealer.set_transition_type(gtk::RevealerTransitionType::SlideRight);
    revealer.set_transition_duration(240);
    revealer.set_reveal_child(true);
    let sidebar_motion = gtk::Fixed::new();
    sidebar_motion.set_size_request(SIDEBAR_WIDTH, -1);
    sidebar_motion.set_hexpand(false);
    sidebar_motion.set_vexpand(true);
    sidebar_motion.put(&content, 0.0, 0.0);
    revealer.set_child(Some(&sidebar_motion));
    revealer.add_css_class("sidebar");

    SidebarParts {
        revealer,
        motion: sidebar_motion,
        content,
        all_button,
        all_label,
        albums_button,
        albums_label,
        artists_button,
        artists_label,
        playlists_button,
        playlists_label,
        liked_button,
        liked_label,
        section_label: section,
    }
}

fn sidebar_row(icon_name: &str, text: &str, active: bool) -> (gtk::Button, gtk::Label) {
    let content = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    content.set_margin_top(7);
    content.set_margin_bottom(7);
    content.set_margin_start(10);
    content.set_margin_end(10);
    content.append(&gtk::Image::from_icon_name(icon_name));
    let label = gtk::Label::new(Some(text));
    label.set_xalign(0.0);
    content.append(&label);

    let button = gtk::Button::new();
    button.set_child(Some(&content));
    button.add_css_class("flat");
    button.add_css_class("sidebar-row");
    if active {
        button.add_css_class("active");
    }
    (button, label)
}
