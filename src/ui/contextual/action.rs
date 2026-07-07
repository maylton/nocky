//! Reusable contextual menu action rows.

use gtk::prelude::*;

pub(crate) const CONTEXTUAL_MENU_ACTION_CLASS: &str = "material-context-menu-action";

pub(crate) fn build_contextual_action(
    label: &str,
    icon_name: &str,
    enabled: bool,
    destructive: bool,
) -> gtk::Button {
    let button = gtk::Button::new();
    button.set_halign(gtk::Align::Fill);
    button.set_valign(gtk::Align::Center);
    button.set_sensitive(enabled);
    button.add_css_class("flat");
    button.add_css_class(CONTEXTUAL_MENU_ACTION_CLASS);
    if destructive {
        button.add_css_class("destructive-action");
    }

    let content = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    content.set_halign(gtk::Align::Fill);
    content.set_valign(gtk::Align::Center);
    content.set_margin_top(8);
    content.set_margin_bottom(8);
    content.set_margin_start(10);
    content.set_margin_end(10);

    let icon = gtk::Image::from_icon_name(icon_name);
    icon.set_pixel_size(16);
    icon.add_css_class("material-context-menu-action-icon");

    let label = gtk::Label::new(Some(label));
    label.set_xalign(0.0);
    label.set_hexpand(true);
    label.add_css_class("material-context-menu-action-label");

    content.append(&icon);
    content.append(&label);
    button.set_child(Some(&content));
    button
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_class_is_stable_for_theme_css() {
        assert_eq!(CONTEXTUAL_MENU_ACTION_CLASS, "material-context-menu-action");
    }
}
