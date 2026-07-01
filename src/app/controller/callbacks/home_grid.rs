//! Responsive multi-row layout for Home card sections.
//!
//! The browser owns card construction. This module only upgrades the mounted
//! horizontal rails after they enter the widget tree, preserving the existing
//! buttons, signals and playback state while replacing the scrolling rail with
//! a responsive `FlowBox`.

use gtk::{gio::prelude::ListModelExt, glib, prelude::*};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

const DEFAULT_HOME_COLUMNS: u32 = 6;

pub(super) fn install(root: &gtk::Stack) {
    let root = root.clone().upcast::<gtk::Widget>();
    let Some(home_stack) = find_home_stack(&root) else {
        return;
    };

    let observed_children = Rc::new(RefCell::new(None));
    observe_visible_home(&home_stack, &observed_children);

    let observed_children_for_notify = observed_children.clone();
    home_stack.connect_notify_local(Some("visible-child"), move |stack, _| {
        observe_visible_home(stack, &observed_children_for_notify);
    });
}

fn find_home_stack(widget: &gtk::Widget) -> Option<gtk::Stack> {
    if let Ok(stack) = widget.clone().downcast::<gtk::Stack>() {
        let mut child = stack.first_child();
        while let Some(current) = child {
            if current.has_css_class("expressive-library-home") {
                return Some(stack);
            }
            child = current.next_sibling();
        }
    }

    let mut child = widget.first_child();
    while let Some(current) = child {
        if let Some(stack) = find_home_stack(&current) {
            return Some(stack);
        }
        child = current.next_sibling();
    }
    None
}

fn observe_visible_home(
    home_stack: &gtk::Stack,
    observed_children: &Rc<RefCell<Option<gtk::gio::ListModel>>>,
) {
    let Some(content) = home_stack.visible_child() else {
        observed_children.borrow_mut().take();
        return;
    };
    if !content.has_css_class("expressive-library-home") {
        observed_children.borrow_mut().take();
        return;
    }

    upgrade_home_carousels(&content);

    let model = content.observe_children();
    let content_weak = content.downgrade();
    model.connect_items_changed(move |_, _, _, _| {
        let Some(content) = content_weak.upgrade() else {
            return;
        };
        glib::idle_add_local_once(move || {
            upgrade_home_carousels(&content);
        });
    });
    observed_children.borrow_mut().replace(model);
}

fn upgrade_home_carousels(root: &gtk::Widget) -> usize {
    let mut converted = 0;
    let mut child = root.first_child();
    while let Some(current) = child {
        child = current.next_sibling();
        converted += upgrade_home_carousels(&current);
    }

    let Ok(scroll) = root.clone().downcast::<gtk::ScrolledWindow>() else {
        return converted;
    };
    if !scroll.has_css_class("material-carousel-scroll") {
        return converted;
    }

    converted + usize::from(upgrade_carousel(&scroll))
}

fn upgrade_carousel(scroll: &gtk::ScrolledWindow) -> bool {
    let Some(child) = scroll.child() else {
        return false;
    };
    let Ok(rail) = child.downcast::<gtk::Box>() else {
        return false;
    };
    if !rail.has_css_class("home-carousel") {
        return false;
    }

    let Some(parent) = scroll.parent() else {
        return false;
    };
    let Ok(section) = parent.downcast::<gtk::Box>() else {
        return false;
    };

    let previous = scroll.prev_sibling();
    let grid = responsive_home_grid();
    while let Some(card) = rail.first_child() {
        rail.remove(&card);
        grid.insert(&card, -1);
    }

    section.remove(scroll);
    section.insert_child_after(&grid, previous.as_ref());
    true
}

fn responsive_home_grid() -> gtk::FlowBox {
    let grid = gtk::FlowBox::new();
    grid.set_selection_mode(gtk::SelectionMode::None);
    grid.set_activate_on_single_click(false);
    grid.set_column_spacing(14);
    grid.set_row_spacing(14);
    grid.set_homogeneous(false);
    grid.set_min_children_per_line(1);
    grid.set_max_children_per_line(DEFAULT_HOME_COLUMNS);
    grid.set_hexpand(true);
    grid.set_valign(gtk::Align::Start);
    grid.add_css_class("home-carousel");
    grid.add_css_class("home-carousel-grid");

    let current_columns = Rc::new(Cell::new(DEFAULT_HOME_COLUMNS));
    grid.connect_notify_local(Some("width"), move |grid, _| {
        let columns = responsive_home_columns(grid.width());
        if current_columns.replace(columns) != columns {
            grid.set_max_children_per_line(columns);
        }
    });
    grid
}

fn responsive_home_columns(width: i32) -> u32 {
    match width {
        ..=639 => 2,
        640..=899 => 3,
        900..=1199 => 4,
        1200..=1599 => DEFAULT_HOME_COLUMNS,
        1600..=1999 => 8,
        _ => 10,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn columns_adapt_around_six_at_normal_desktop_width() {
        assert_eq!(responsive_home_columns(480), 2);
        assert_eq!(responsive_home_columns(800), 3);
        assert_eq!(responsive_home_columns(1024), 4);
        assert_eq!(responsive_home_columns(1280), 6);
        assert_eq!(responsive_home_columns(1700), 8);
        assert_eq!(responsive_home_columns(2200), 10);
    }
}
