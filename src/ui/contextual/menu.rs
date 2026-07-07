//! Reusable Material Expressive contextual menu container.

use gtk::prelude::*;

pub(crate) const CONTEXTUAL_MENU_CLASS: &str = "material-context-menu";

pub(crate) struct MaterialContextMenu {
    button: gtk::MenuButton,
    popover: gtk::Popover,
    content: gtk::Box,
}

impl MaterialContextMenu {
    pub(crate) fn new(tooltip_text: &str) -> Self {
        let popover = gtk::Popover::new();
        popover.set_has_arrow(false);
        popover.set_autohide(true);
        popover.add_css_class(CONTEXTUAL_MENU_CLASS);

        let content = gtk::Box::new(gtk::Orientation::Vertical, 4);
        content.set_margin_top(8);
        content.set_margin_bottom(8);
        content.set_margin_start(8);
        content.set_margin_end(8);
        content.add_css_class("material-context-menu-content");
        popover.set_child(Some(&content));

        let icon = gtk::Image::from_icon_name("view-more-symbolic");
        icon.set_pixel_size(16);
        icon.add_css_class("material-context-menu-button-icon");

        let button = gtk::MenuButton::new();
        button.set_child(Some(&icon));
        button.set_popover(Some(&popover));
        button.set_tooltip_text(Some(tooltip_text));
        button.set_halign(gtk::Align::Center);
        button.set_valign(gtk::Align::Center);
        button.add_css_class("flat");
        button.add_css_class("circular");
        button.add_css_class("material-context-menu-button");

        Self {
            button,
            popover,
            content,
        }
    }

    pub(crate) fn button(&self) -> &gtk::MenuButton {
        &self.button
    }

    pub(crate) fn popover(&self) -> &gtk::Popover {
        &self.popover
    }

    pub(crate) fn append_action(&self, action: &gtk::Button) {
        self.content.append(action);
    }

    pub(crate) fn append_separator(&self) {
        let separator = gtk::Separator::new(gtk::Orientation::Horizontal);
        separator.set_margin_top(4);
        separator.set_margin_bottom(4);
        separator.add_css_class("material-context-menu-separator");
        self.content.append(&separator);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn menu_class_is_stable_for_theme_css() {
        assert_eq!(CONTEXTUAL_MENU_CLASS, "material-context-menu");
    }
}
