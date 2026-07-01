use gtk::prelude::*;

pub(super) fn apply(root: &gtk::Stack) {
    root.add_css_class("adaptive-home-density");
}
