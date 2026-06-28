use gtk::{cairo, glib, prelude::*};
use std::{cell::Cell, f64::consts::TAU, rc::Rc};

const DEFAULT_SIZE: i32 = 22;
const TRACK_ALPHA: f64 = 0.22;
const ACTIVE_ALPHA: f64 = 0.96;
const STROKE_WIDTH: f64 = 4.0;
const ACTIVE_ARC_SWEEP: f64 = TAU * 0.34;
const TRACK_GAP: f64 = TAU * 0.055;
const ROTATION_STEP: f64 = 0.075;

#[derive(Clone)]
pub struct ExpressiveLoadingIndicator {
    area: gtk::DrawingArea,
}

impl ExpressiveLoadingIndicator {
    pub fn new() -> Self {
        Self::with_size(DEFAULT_SIZE)
    }

    pub fn with_size(size: i32) -> Self {
        let area = gtk::DrawingArea::new();
        area.set_size_request(size, size);
        area.add_css_class("expressive-loading-indicator");

        let phase = Rc::new(Cell::new(0.0));

        {
            let phase = phase.clone();
            area.set_draw_func(move |widget, context, width, height| {
                draw_indicator(widget, context, width, height, phase.get());
            });
        }

        {
            let phase = phase.clone();
            area.add_tick_callback(move |widget, _| {
                phase.set((phase.get() + ROTATION_STEP) % TAU);
                widget.queue_draw();
                glib::ControlFlow::Continue
            });
        }

        Self { area }
    }

    pub fn widget(&self) -> &gtk::DrawingArea {
        &self.area
    }
}

fn draw_indicator(
    widget: &gtk::DrawingArea,
    context: &cairo::Context,
    width: i32,
    height: i32,
    phase: f64,
) {
    if width <= 0 || height <= 0 {
        return;
    }

    let color = widget.color();
    let red = color.red() as f64;
    let green = color.green() as f64;
    let blue = color.blue() as f64;

    let size = width.min(height) as f64;
    let center_x = width as f64 / 2.0;
    let center_y = height as f64 / 2.0;
    let radius = indicator_radius(size);
    let start = phase - TAU / 4.0;
    let end = start + active_arc_sweep();
    let track_start = end + TRACK_GAP;
    let track_end = start + TAU - TRACK_GAP;

    context.set_line_cap(cairo::LineCap::Round);
    context.set_line_width(STROKE_WIDTH);

    if track_start < track_end {
        context.new_path();
        context.set_source_rgba(red, green, blue, TRACK_ALPHA);
        context.arc(center_x, center_y, radius, track_start, track_end);
        let _ = context.stroke();
    }

    context.new_path();
    context.set_source_rgba(red, green, blue, ACTIVE_ALPHA);
    context.arc(center_x, center_y, radius, start, end);
    let _ = context.stroke();
}

fn indicator_radius(size: f64) -> f64 {
    ((size - STROKE_WIDTH) / 2.0).max(1.0)
}

fn active_arc_sweep() -> f64 {
    ACTIVE_ARC_SWEEP
}

#[cfg(test)]
fn painted_arc_gap() -> f64 {
    TRACK_GAP
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loading_indicator_geometry_fits_compact_buttons() {
        assert_eq!(DEFAULT_SIZE, 22);
        assert_eq!(STROKE_WIDTH, 4.0);
        assert!(indicator_radius(DEFAULT_SIZE as f64) <= 9.0);
    }

    #[test]
    fn loading_indicator_arc_keeps_an_indeterminate_gap() {
        let sweep = active_arc_sweep();

        assert!(sweep < TAU * 0.5);
        assert!(sweep > TAU * 0.25);
    }

    #[test]
    fn loading_indicator_track_keeps_material_gap() {
        let gap = painted_arc_gap();

        assert!(gap > TAU * 0.04);
        assert!(gap < TAU * 0.08);
    }
}
