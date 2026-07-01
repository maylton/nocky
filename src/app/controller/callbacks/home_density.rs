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

    let root_widget: &gtk::Widget = root.upcast_ref();
    for stack in descendant_stacks(root_widget) {
        let changed = refresh.clone();
        stack.connect_notify_local(Some("visible-child"), move |_, _| {
            schedule(changed.clone());
        });
    }

    schedule(refresh);
}

fn schedule(refresh: Rc<dyn Fn()>) {
    glib::idle_add_local_once(move || {
        refresh();
    });
}

fn descendant_stacks(root: &gtk::Widget) -> Vec<gtk::Stack> {
    descendants(root)
        .into_iter()
        .filter_map(|widget| widget.downcast::<gtk::Stack>().ok())
        .collect()
}

fn descendants(root: &gtk::Widget) -> Vec<gtk::Widget> {
    let mut result = Vec::new();
    let mut pending = vec![root.clone()];

    while let Some(widget) = pending.pop() {
        let mut child = widget.first_child();
        while let Some(current) = child {
            child = current.next_sibling();
            pending.push(current);
        }
        result.push(widget);
    }

    result
}
