use gtk::prelude::*;

pub(super) fn install(root: &gtk::Stack) {
    root.add_css_class("adaptive-home-density");
}
