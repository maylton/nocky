use super::track_rows;
use gtk::prelude::*;
use std::collections::VecDeque;

const COMPACT_OUTER_WIDTH: i32 = 172;
const COMPACT_OUTER_HEIGHT: i32 = 210;
const COMPACT_CARD_WIDTH: i32 = 152;
const COMPACT_CARD_HEIGHT: i32 = 194;
const COMPACT_ARTWORK_SIZE: i32 = 136;
const COMPACT_ACTION_SIZE: i32 = 34;

pub(super) fn apply(root: &gtk::Stack) {
    let root_widget: &gtk::Widget = root.upcast_ref();
    let mut featured_assigned = false;

    for section in descendants_in_visual_order(root_widget) {
        if !section.has_css_class("home-section")
            || section.has_css_class("youtube-home-chip-section")
        {
            continue;
        }

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

        let grid_width = grid.width().max(1);
        clear_presentation_classes(&section);

        if track_rows::section_is_track_only(&cards) {
            section.add_css_class("home-section-track-rows");
            track_rows::configure_grid(&grid, grid_width);
            for card in cards {
                track_rows::apply_card(&card);
            }
            continue;
        }

        if !featured_assigned {
            featured_assigned = true;
            section.add_css_class("home-section-featured");
            grid.set_halign(gtk::Align::Fill);
            continue;
        }

        section.add_css_class("home-section-compact");
        configure_compact_grid(&grid, grid_width);
        for card in cards {
            compact_card(&card);
        }
    }
}

fn clear_presentation_classes(section: &gtk::Widget) {
    for class_name in [
        "home-section-featured",
        "home-section-compact",
        "home-section-track-rows",
    ] {
        section.remove_css_class(class_name);
    }
}

fn configure_compact_grid(grid: &gtk::FlowBox, width: i32) {
    grid.set_homogeneous(false);
    grid.set_min_children_per_line(2);
    grid.set_max_children_per_line(compact_columns(width));
    grid.set_column_spacing(12);
    grid.set_row_spacing(14);
    grid.set_halign(gtk::Align::Start);
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
    root.set_size_request(COMPACT_OUTER_WIDTH, COMPACT_OUTER_HEIGHT);
    root.set_hexpand(false);
    root.set_halign(gtk::Align::Start);

    for widget in descendants_in_visual_order(root) {
        if widget.has_css_class("home-card-context-overlay")
            || widget.has_css_class("home-card-button")
        {
            widget.set_size_request(COMPACT_OUTER_WIDTH, COMPACT_OUTER_HEIGHT);
            widget.set_hexpand(false);
            widget.set_halign(gtk::Align::Start);
        }

        if widget.has_css_class("home-card") {
            widget.set_size_request(COMPACT_CARD_WIDTH, COMPACT_CARD_HEIGHT);
            widget.set_hexpand(false);
            widget.set_halign(gtk::Align::Start);
            widget.add_css_class("home-card-compact");
        }

        if widget.has_css_class("collection-artwork") {
            resize_artwork(&widget, COMPACT_ARTWORK_SIZE);
        }

        if widget.has_css_class("collection-card-detail") {
            widget.set_visible(false);
        }

        if widget.has_css_class("collection-card-title")
            || widget.has_css_class("expressive-card-subtitle")
        {
            if let Ok(label) = widget.clone().downcast::<gtk::Label>() {
                label.set_width_chars(14);
                label.set_max_width_chars(14);
            }
        }

        if widget.has_css_class("collection-card-context-action")
            || widget.has_css_class("collection-card-overflow-button")
        {
            widget.set_size_request(COMPACT_ACTION_SIZE, COMPACT_ACTION_SIZE);
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
    descendants_in_visual_order(root)
        .into_iter()
        .find(|widget| widget.has_css_class(class_name))
}

fn descendants_in_visual_order(root: &gtk::Widget) -> Vec<gtk::Widget> {
    let mut result = Vec::new();
    let mut pending = VecDeque::from([root.clone()]);

    while let Some(widget) = pending.pop_front() {
        let mut child = widget.first_child();
        while let Some(current) = child {
            child = current.next_sibling();
            pending.push_back(current);
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
    fn compact_breakpoints_scale_with_grid_width() {
        assert_eq!(compact_columns(420), 2);
        assert_eq!(compact_columns(600), 3);
        assert_eq!(compact_columns(840), 4);
        assert_eq!(compact_columns(1100), 6);
        assert_eq!(compact_columns(1440), 8);
        assert_eq!(compact_columns(1800), 10);
        assert_eq!(compact_columns(2200), 12);
    }
}
