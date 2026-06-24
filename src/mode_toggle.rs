// nocky_md3_repeat_shuffle_toggles_v1
use gtk::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ModeToggleKind {
    Shuffle,
    RepeatOne,
}

pub(crate) fn new_mode_toggle(
    icon_name: &str,
    tooltip: &str,
    kind: ModeToggleKind,
) -> gtk::ToggleButton {
    let icon = gtk::Image::from_icon_name(icon_name);
    icon.set_pixel_size(18);
    icon.set_can_target(false);
    icon.add_css_class("mode-toggle-icon");

    let overlay = gtk::Overlay::new();
    overlay.set_child(Some(&icon));
    overlay.set_can_target(false);
    overlay.add_css_class("mode-toggle-overlay");

    if kind == ModeToggleKind::RepeatOne {
        let badge = gtk::Label::new(Some("1"));
        badge.set_halign(gtk::Align::End);
        badge.set_valign(gtk::Align::Start);
        badge.set_margin_top(3);
        badge.set_margin_end(3);
        badge.set_can_target(false);
        badge.add_css_class("mode-toggle-badge");
        overlay.add_overlay(&badge);
    }

    let button = gtk::ToggleButton::new();
    button.set_child(Some(&overlay));
    button.set_tooltip_text(Some(tooltip));
    button.set_size_request(40, 40);
    button.set_hexpand(false);
    button.set_vexpand(false);
    button.set_halign(gtk::Align::Center);
    button.set_valign(gtk::Align::Center);
    button.add_css_class("flat");
    button.add_css_class("playback-mode-toggle");

    match kind {
        ModeToggleKind::Shuffle => {
            button.add_css_class("shuffle-mode-toggle");
        }
        ModeToggleKind::RepeatOne => {
            button.add_css_class("repeat-mode-toggle");
        }
    }

    button
}
