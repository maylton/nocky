use gtk::prelude::*;

pub(super) fn install(root: &gtk::Widget) {
    root.add_css_class("adaptive-home-density");
}
