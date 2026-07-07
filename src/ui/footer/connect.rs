//! Footer entry point for Nocky Connect.
//!
//! The actual transfer and LAN discovery flow belongs to the application
//! controller/connect layer. This module only owns the small footer button so
//! the regular footer utility layout remains simple.

use gtk::prelude::*;

pub(crate) const FOOTER_CONNECT_WIDTH_DELTA: i32 = 48;

pub(crate) fn build_footer_connect_button() -> gtk::Button {
    let icon = gtk::Image::from_icon_name("smartphone-symbolic");
    icon.add_css_class("footer-connect-icon");
    icon.set_pixel_size(16);
    icon.set_valign(gtk::Align::Center);
    icon.set_halign(gtk::Align::Center);

    let indicator = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    indicator.add_css_class("footer-connect-indicator");
    indicator.set_valign(gtk::Align::Center);
    indicator.set_halign(gtk::Align::Center);
    indicator.append(&icon);

    let button = gtk::Button::builder()
        .tooltip_text("Nocky Connect")
        .action_name("app.nocky-connect")
        .child(&indicator)
        .build();
    button.add_css_class("flat");
    button.add_css_class("footer-control");
    button.add_css_class("footer-utility-action");
    button.add_css_class("footer-connect-button");
    button.set_valign(gtk::Align::Center);
    button
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn footer_connect_width_delta_matches_one_control_slot() {
        assert_eq!(FOOTER_CONNECT_WIDTH_DELTA, 48);
    }
}
