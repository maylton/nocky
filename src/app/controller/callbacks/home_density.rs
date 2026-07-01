use gtk::{glib, prelude::*};
use std::rc::Rc;

pub(super) fn install(root: &gtk::Stack) {
    let weak = root.downgrade();
    let refresh: Rc<dyn Fn()> = Rc::new(move || {
        if let Some(root) = weak.upgrade() {
            root.add_css_class("adaptive-home-density");
        }
    });

    let resized = refresh.clone();
    root.connect_notify_local(Some("width"), move |_, _| {
        schedule(resized.clone());
    });

    schedule(refresh);
}

fn schedule(refresh: Rc<dyn Fn()>) {
    glib::idle_add_local_once(move || {
        refresh();
    });
}
