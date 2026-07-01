//! Responsive layout helpers for Home presentation.

use gtk::prelude::*;
use std::{cell::Cell, rc::Rc};

const BASE_HOME_COLUMNS: u32 = 6;

pub(crate) fn adaptive_home_grid() -> gtk::FlowBox {
    let grid = gtk::FlowBox::new();
    grid.set_selection_mode(gtk::SelectionMode::None);
    grid.set_activate_on_single_click(false);
    grid.set_column_spacing(14);
    grid.set_row_spacing(14);
    grid.set_homogeneous(false);
    grid.set_min_children_per_line(1);
    grid.set_max_children_per_line(BASE_HOME_COLUMNS);
    grid.set_hexpand(true);
    grid.set_valign(gtk::Align::Start);
    grid.add_css_class("home-carousel");
    grid.add_css_class("home-carousel-grid");

    let current_columns = Rc::new(Cell::new(BASE_HOME_COLUMNS));
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
        1200..=1599 => BASE_HOME_COLUMNS,
        1600..=1999 => 8,
        _ => 10,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adapts_around_six_desktop_columns() {
        assert_eq!(responsive_home_columns(1280), 6);
        assert_eq!(responsive_home_columns(900), 4);
        assert_eq!(responsive_home_columns(1700), 8);
    }
}
