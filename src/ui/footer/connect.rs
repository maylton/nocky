//! Footer entry point for Nocky Connect.
//!
//! The actual transfer and LAN discovery flow belongs to the application
//! controller/connect layer. This module only owns the small footer button so
//! the regular footer utility layout remains simple.

use gtk::prelude::*;

pub(crate) const FOOTER_CONNECT_WIDTH_DELTA: i32 = 48;

pub(crate) fn build_footer_connect_button() -> gtk::Button {
    let button = gtk::Button::builder()
        .icon_name("network-workgroup-symbolic")
        .tooltip_text("Nocky Connect")
        .action_name("app.nocky-connect")
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
