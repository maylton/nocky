use gtk::prelude::*;

const ROW_MIN_WIDTH: i32 = 300;
const ROW_HEIGHT: i32 = 82;
const CARD_HEIGHT: i32 = 72;
const ARTWORK_SIZE: i32 = 56;
const ACTION_SIZE: i32 = 36;

pub(super) fn section_is_track_only(cards: &[gtk::Widget]) -> bool {
    !cards.is_empty() && cards.iter().all(card_is_track)
}

pub(super) fn configure_grid(grid: &gtk::FlowBox, width: i32) {
    grid.set_homogeneous(true);
    grid.set_min_children_per_line(1);
    grid.set_max_children_per_line(columns(width));
    grid.set_column_spacing(14);
    grid.set_row_spacing(8);
    grid.set_halign(gtk::Align::Fill);
}

pub(super) fn apply_card(root: &gtk::Widget) {
    root.set_size_request(ROW_MIN_WIDTH, ROW_HEIGHT);
    root.set_hexpand(true);
    root.set_halign(gtk::Align::Fill);

    for widget in descendants(root) {
        if widget.has_css_class("home-card-context-overlay")
            || widget.has_css_class("home-card-button")
        {
            widget.set_size_request(ROW_MIN_WIDTH, ROW_HEIGHT);
            widget.set_hexpand(true);
            widget.set_halign(gtk::Align::Fill);
        }

        if widget.has_css_class("collection-card-context-action")
            || widget.has_css_class("collection-card-overflow-button")
        {
            widget.set_size_request(ACTION_SIZE, ACTION_SIZE);
            widget.set_margin_top(0);
            widget.set_margin_start(8);
            widget.set_margin_end(8);
            widget.set_valign(gtk::Align::Center);
        }
    }

    let Some(card) =
        find_class(root, "home-card").and_then(|widget| widget.downcast::<gtk::Box>().ok())
    else {
        return;
    };

    if !card.has_css_class("home-track-card") {
        arrange_content(&card);
    }

    card.set_size_request(ROW_MIN_WIDTH - 20, CARD_HEIGHT);
    card.set_hexpand(true);
    card.set_vexpand(false);
    card.set_halign(gtk::Align::Fill);
    card.set_valign(gtk::Align::Center);
    card.set_margin_top(4);
    card.set_margin_bottom(4);
    card.set_margin_start(6);
    card.set_margin_end(ACTION_SIZE + 12);
    card.add_css_class("home-track-card");

    if let Some(artwork) = find_class(root, "collection-artwork") {
        resize_artwork(&artwork, ARTWORK_SIZE);
    }
}

fn columns(width: i32) -> u32 {
    match width {
        ..=699 => 1,
        700..=1399 => 2,
        _ => 3,
    }
}

fn card_is_track(root: &gtk::Widget) -> bool {
    descendants(root).into_iter().any(|widget| {
        let name = widget.widget_name().to_string();
        is_track_playback_name(&name)
    })
}

fn is_track_playback_name(name: &str) -> bool {
    name.starts_with("home-play-card:") && name.contains("track:")
}

fn arrange_content(card: &gtk::Box) {
    let card_widget: gtk::Widget = card.clone().upcast();
    let children = direct_children(&card_widget);
    let Some(artwork) = children.first().cloned() else {
        return;
    };

    artwork.set_size_request(ARTWORK_SIZE, ARTWORK_SIZE);
    artwork.set_hexpand(false);
    artwork.set_vexpand(false);
    artwork.set_halign(gtk::Align::Start);
    artwork.set_valign(gtk::Align::Center);

    let text = gtk::Box::new(gtk::Orientation::Vertical, 2);
    text.set_hexpand(true);
    text.set_halign(gtk::Align::Fill);
    text.set_valign(gtk::Align::Center);
    text.add_css_class("home-track-card-text");

    for child in children.into_iter().skip(1) {
        card.remove(&child);

        if child.has_css_class("collection-card-detail") {
            child.set_visible(false);
        }

        if let Ok(label) = child.clone().downcast::<gtk::Label>() {
            label.set_width_chars(18);
            label.set_max_width_chars(36);
            label.set_xalign(0.0);
            label.set_single_line_mode(true);
        }

        text.append(&child);
    }

    card.set_orientation(gtk::Orientation::Horizontal);
    card.set_spacing(10);
    card.append(&text);
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
    use super::{columns, is_track_playback_name};

    #[test]
    fn rows_preserve_readable_columns() {
        assert_eq!(columns(600), 1);
        assert_eq!(columns(1000), 2);
        assert_eq!(columns(1800), 3);
    }

    #[test]
    fn only_track_cards_match() {
        assert!(is_track_playback_name(
            "home-play-card:youtube:track:abc123"
        ));
        assert!(!is_track_playback_name(
            "home-play-card:youtube:playlist:PL123"
        ));
        assert!(!is_track_playback_name(
            "home-play-control:youtube:track:abc123"
        ));
    }
}
