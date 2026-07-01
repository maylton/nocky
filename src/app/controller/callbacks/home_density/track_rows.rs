use gtk::prelude::*;

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

pub(super) fn apply_card(_root: &gtk::Widget) {}

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
