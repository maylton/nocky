use gtk::{glib, prelude::*};
use std::rc::Rc;

pub(super) fn install(root: &gtk::Widget) {
    let weak = root.downgrade();
    let refresh: Rc<dyn Fn()> = Rc::new(move || {
        if let Some(root) = weak.upgrade() {
            apply(&root);
        }
    });

    let resize_refresh = refresh.clone();
    root.connect_notify_local(Some("width"), move |_, _| {
        schedule(resize_refresh.clone());
    });

    for stack in descendants(root)
        .into_iter()
        .filter_map(|widget| widget.downcast::<gtk::Stack>().ok())
    {
        let stack_refresh = refresh.clone();
        stack.connect_notify_local(Some("visible-child"), move |_, _| {
            schedule(stack_refresh.clone());
        });
    }

    schedule(refresh);
}

fn schedule(refresh: Rc<dyn Fn()>) {
    glib::idle_add_local_once(move || refresh());
}

fn apply(root: &gtk::Widget) {
    let mut featured = false;
    let width = root.width().max(1);

    for section in descendants(root).into_iter().filter(is_media_section) {
        let Some(grid) = direct_flow_box(&section) else {
            continue;
        };
        let grid_widget: gtk::Widget = grid.clone().upcast();
        let cards = direct_children(&grid_widget)
            .into_iter()
            .filter(|child| find_class(child, "home-card-button").is_some())
            .collect::<Vec<_>>();

        if cards.is_empty() {
            continue;
        }
        if !featured {
            featured = true;
            section.add_css_class("home-section-featured");
            section.remove_css_class("home-section-compact");
            continue;
        }

        section.remove_css_class("home-section-featured");
        section.add_css_class("home-section-compact");
        grid.set_homogeneous(false);
        grid.set_min_children_per_line(2);
        grid.set_max_children_per_line(compact_columns(width));
        grid.set_column_spacing(12);
        grid.set_row_spacing(14);
        cards.iter().for_each(compact_card);
    }
}

fn is_media_section(widget: &gtk::Widget) -> bool {
    widget.has_css_class("home-section")
        && !widget.has_css_class("youtube-home-chip-section")
}

fn compact_columns(width: i32) -> u32 {
    match width {
        ..=479 => 2,
        480..=719 => 3,
        720..=959 => 4,
        960..=1279 => 6,
        1280..=1599 => 8,
        1600..=1999 => 10,
        _ => 12,
    }
}

fn compact_card(root: &gtk::Widget) {
    root.set_size_request(168, 196);
    root.set_hexpand(false);
    root.set_halign(gtk::Align::Start);

    for widget in descendants(root) {
        if widget.has_css_class("home-card-context-overlay")
            || widget.has_css_class("home-card-button")
        {
            widget.set_size_request(168, 196);
            widget.set_hexpand(false);
            widget.set_halign(gtk::Align::Start);
        } else if widget.has_css_class("collection-card-detail") {
            widget.set_visible(false);
        } else if widget.has_css_class("collection-artwork") {
            resize_artwork(&widget, 128);
        } else if widget.has_css_class("home-card") {
            widget.set_size_request(148, 180);
            widget.set_hexpand(false);
            widget.set_halign(gtk::Align::Start);
            widget.add_css_class("home-card-compact");
        }

        if widget.has_css_class("collection-card-context-action")
            || widget.has_css_class("collection-card-overflow-button")
        {
            widget.set_size_request(34, 34);
            widget.set_margin_top(8);
            widget.set_margin_start(8);
            widget.set_margin_end(8);
        }
    }
}

fn resize_artwork(artwork: &gtk::Widget, size: i32) {
    artwork.set_size_request(size, size);
    artwork.set_hexpand(false);
    artwork.set_vexpand(false);
    for child in direct_children(artwork) {
        child.set_size_request(size, size);
        if let Ok(image) = child.clone().downcast::<gtk::Image>() {
            image.set_pixel_size(size / 3);
        }
        if let Ok(picture) = child.downcast::<gtk::Picture>() {
            picture.set_size_request(size, size);
        }
    }
}

fn direct_flow_box(section: &gtk::Widget) -> Option<gtk::FlowBox> {
    direct_children(section)
        .into_iter()
        .find_map(|child| child.downcast::<gtk::FlowBox>().ok())
}

fn find_class(root: &gtk::Widget, class_name: &str) -> Option<gtk::Widget> {
    descendants(root)
        .into_iter()
        .find(|widget| widget.has_css_class(class_name))
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

fn direct_children(root: &gtk::Widget) -> Vec<gtk::Widget> {
    let mut result = Vec::new();
    let mut child = root.first_child();
    while let Some(current) = child {
        child = current.next_sibling();
        result.push(current);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::compact_columns;

    #[test]
    fn compact_breakpoints_scale_with_desktop_width() {
        assert_eq!(compact_columns(420), 2);
        assert_eq!(compact_columns(600), 3);
        assert_eq!(compact_columns(840), 4);
        assert_eq!(compact_columns(1100), 6);
        assert_eq!(compact_columns(1440), 8);
        assert_eq!(compact_columns(1800), 10);
        assert_eq!(compact_columns(2200), 12);
    }
}
