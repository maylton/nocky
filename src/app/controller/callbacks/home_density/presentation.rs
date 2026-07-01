use gtk::prelude::*;

pub(super) fn apply(root: &gtk::Stack) {
    let root_widget: &gtk::Widget = root.upcast_ref();
    let width = root.width().max(1);
    let mut featured_assigned = false;

    for section in descendants(root_widget) {
        if !section.has_css_class("home-section")
            || section.has_css_class("youtube-home-chip-section")
        {
            continue;
        }
        let Some(grid) = direct_flow_box(&section) else {
            continue;
        };
        if grid.first_child().is_none() {
            continue;
        }

        if !featured_assigned {
            featured_assigned = true;
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
    }
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

fn direct_flow_box(section: &gtk::Widget) -> Option<gtk::FlowBox> {
    let mut child = section.first_child();
    while let Some(current) = child {
        child = current.next_sibling();
        if let Ok(grid) = current.downcast::<gtk::FlowBox>() {
            return Some(grid);
        }
    }
    None
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
