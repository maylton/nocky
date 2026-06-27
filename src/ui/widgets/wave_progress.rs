use gtk::{cairo, glib, prelude::*};
use std::{
    cell::{Cell, RefCell},
    f64::consts::TAU,
    rc::Rc,
};

type SeekCallback = Box<dyn Fn(f64)>;

const HEIGHT_REQUEST: i32 = 24;
const EDGE_PADDING: f64 = 6.0;
const TRACK_THICKNESS: f64 = 8.0;
const TRACK_RADIUS: f64 = TRACK_THICKNESS / 2.0;
const TRACK_ALPHA: f64 = 0.22;
const ACTIVE_ALPHA: f64 = 0.96;
const ACTIVE_TRACK_GAP: f64 = 6.0;
const STOP_INDICATOR_RADIUS: f64 = 4.0;
const STOP_INDICATOR_ALPHA: f64 = 0.34;
const STOP_TRACK_GAP: f64 = 4.0;
const WAVE_AMPLITUDE: f64 = 2.45;
const WAVELENGTH: f64 = 20.0;
const WAVE_STEP: f64 = 1.25;
const BRIDGE_STEP: f64 = 0.75;

#[derive(Clone)]
pub struct WaveProgress {
    area: gtk::DrawingArea,
    fraction: Rc<Cell<f64>>,
    playing: Rc<Cell<bool>>,
    callbacks: Rc<RefCell<Vec<SeekCallback>>>,
}

impl WaveProgress {
    pub fn new() -> Self {
        let area = gtk::DrawingArea::new();
        area.set_hexpand(true);
        area.set_height_request(HEIGHT_REQUEST);
        area.set_cursor_from_name(Some("pointer"));
        area.add_css_class("footer-wave-progress");

        let fraction = Rc::new(Cell::new(0.0));
        let phase = Rc::new(Cell::new(0.0));
        let playing = Rc::new(Cell::new(false));
        let callbacks = Rc::new(RefCell::new(Vec::<SeekCallback>::new()));

        {
            let fraction = fraction.clone();
            let phase = phase.clone();
            area.set_draw_func(move |widget, context, width, height| {
                draw_wave(widget, context, width, height, fraction.get(), phase.get());
            });
        }

        {
            let fraction = fraction.clone();
            let callbacks = callbacks.clone();
            let area_clone = area.clone();
            let click = gtk::GestureClick::new();
            click.set_button(1);
            click.connect_pressed(move |_, _, x, _| {
                emit_seek(&area_clone, &fraction, &callbacks, x);
            });
            area.add_controller(click);
        }

        {
            let fraction = fraction.clone();
            let callbacks = callbacks.clone();
            let area_clone = area.clone();
            let origin = Rc::new(Cell::new(0.0));
            let drag = gtk::GestureDrag::new();

            let origin_begin = origin.clone();
            let begin_area = area_clone.clone();
            let begin_fraction = fraction.clone();
            let begin_callbacks = callbacks.clone();
            drag.connect_drag_begin(move |_, x, _| {
                origin_begin.set(x);
                emit_seek(&begin_area, &begin_fraction, &begin_callbacks, x);
            });

            drag.connect_drag_update(move |_, offset_x, _| {
                emit_seek(&area_clone, &fraction, &callbacks, origin.get() + offset_x);
            });
            area.add_controller(drag);
        }

        {
            let phase = phase.clone();
            let playing = playing.clone();
            area.add_tick_callback(move |widget, _| {
                if playing.get() {
                    phase.set((phase.get() + 0.018) % TAU);
                    widget.queue_draw();
                }
                glib::ControlFlow::Continue
            });
        }

        Self {
            area,
            fraction,
            playing,
            callbacks,
        }
    }

    pub fn widget(&self) -> &gtk::DrawingArea {
        &self.area
    }

    pub fn set_fraction(&self, fraction: f64) {
        let fraction = fraction.clamp(0.0, 1.0);
        if (self.fraction.get() - fraction).abs() > 0.000_1 {
            self.fraction.set(fraction);
            self.area.queue_draw();
        }
    }

    pub fn set_playing(&self, playing: bool) {
        self.playing.set(playing);
        self.area.queue_draw();
    }

    pub fn connect_seek<F>(&self, callback: F)
    where
        F: Fn(f64) + 'static,
    {
        self.callbacks.borrow_mut().push(Box::new(callback));
    }
}

fn emit_seek(
    area: &gtk::DrawingArea,
    fraction: &Cell<f64>,
    callbacks: &RefCell<Vec<SeekCallback>>,
    x: f64,
) {
    let width = area.width().max(1) as f64;
    let stop_x = progress_stop_x(width);
    let usable_width = (stop_x - EDGE_PADDING).max(1.0);
    let value = ((x - EDGE_PADDING) / usable_width).clamp(0.0, 1.0);
    fraction.set(value);
    area.queue_draw();
    for callback in callbacks.borrow().iter() {
        callback(value);
    }
}

fn draw_wave(
    widget: &gtk::DrawingArea,
    context: &cairo::Context,
    width: i32,
    height: i32,
    fraction: f64,
    phase: f64,
) {
    if width <= 0 || height <= 0 {
        return;
    }

    let color = widget.color();
    let red = color.red() as f64;
    let green = color.green() as f64;
    let blue = color.blue() as f64;

    let width = width as f64;
    let middle = height as f64 / 2.0;
    let fraction = fraction.clamp(0.0, 1.0);

    let stop_x = progress_stop_x(width);
    let usable_width = (stop_x - EDGE_PADDING).max(1.0);
    let progress_x = EDGE_PADDING + usable_width * fraction;
    let tau = std::f64::consts::TAU;

    context.set_line_cap(cairo::LineCap::Round);
    context.set_line_join(cairo::LineJoin::Round);

    let track_start = inactive_track_start(progress_x);
    let track_end = inactive_track_end(stop_x);
    if track_start < track_end {
        context.new_path();
        context.set_source_rgba(red, green, blue, TRACK_ALPHA);
        context.set_line_width(TRACK_THICKNESS);
        context.move_to(track_start, middle);
        context.line_to(track_end, middle);
        let _ = context.stroke();
    }

    if fraction < 0.995 {
        context.new_path();
        context.set_source_rgba(red, green, blue, STOP_INDICATOR_ALPHA);
        context.arc(stop_x, middle, STOP_INDICATOR_RADIUS, 0.0, tau);
        let _ = context.fill();
    }

    if fraction > 0.0 {
        let bridge_length = ACTIVE_TRACK_GAP.min(progress_x - EDGE_PADDING);
        let bridge_start = progress_x - bridge_length;

        context.new_path();
        context.set_source_rgba(red, green, blue, ACTIVE_ALPHA);
        context.set_line_width(TRACK_THICKNESS);

        let wave_y = |x: f64| middle + ((x / WAVELENGTH) * tau + phase).sin() * WAVE_AMPLITUDE;

        let wave_slope =
            |x: f64| ((x / WAVELENGTH) * tau + phase).cos() * WAVE_AMPLITUDE * tau / WAVELENGTH;

        let start_y = wave_y(EDGE_PADDING);
        context.move_to(EDGE_PADDING, start_y);

        let mut x = EDGE_PADDING + WAVE_STEP;

        while x < bridge_start {
            context.line_to(x, wave_y(x));
            x += WAVE_STEP;
        }

        if bridge_length > 0.01 {
            let y0 = wave_y(bridge_start) - middle;
            let slope0 = wave_slope(bridge_start);

            context.line_to(bridge_start, middle + y0);

            let mut bridge_x = bridge_start + BRIDGE_STEP;

            while bridge_x < progress_x {
                let t = ((bridge_x - bridge_start) / bridge_length).clamp(0.0, 1.0);

                let t2 = t * t;
                let t3 = t2 * t;

                let h00 = 2.0 * t3 - 3.0 * t2 + 1.0;
                let h10 = t3 - 2.0 * t2 + t;

                let offset = h00 * y0 + h10 * slope0 * bridge_length;

                context.line_to(bridge_x, middle + offset);
                bridge_x += BRIDGE_STEP;
            }
        }

        context.line_to(progress_x, middle);
        let _ = context.stroke();
    }
}

fn inactive_track_start(progress_x: f64) -> f64 {
    progress_x + ACTIVE_TRACK_GAP + TRACK_RADIUS * 2.0
}

fn progress_stop_x(width: f64) -> f64 {
    width - EDGE_PADDING - STOP_INDICATOR_RADIUS
}

fn inactive_track_end(stop_x: f64) -> f64 {
    stop_x - STOP_INDICATOR_RADIUS - STOP_TRACK_GAP - TRACK_RADIUS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expressive_progress_geometry_keeps_material_cues() {
        assert_eq!(HEIGHT_REQUEST, 24);
        assert_eq!(EDGE_PADDING, 6.0);
        assert_eq!(TRACK_THICKNESS, 8.0);
        assert_eq!(ACTIVE_TRACK_GAP, 6.0);
        assert_eq!(STOP_INDICATOR_RADIUS, 4.0);
        assert_eq!(STOP_TRACK_GAP, 4.0);
    }

    #[test]
    fn inactive_track_gap_accounts_for_round_caps() {
        let progress_x = 48.0;
        let painted_active_end = progress_x + TRACK_RADIUS;
        let painted_track_start = inactive_track_start(progress_x) - TRACK_RADIUS;

        assert_eq!(painted_track_start - painted_active_end, ACTIVE_TRACK_GAP);
    }

    #[test]
    fn inactive_track_stops_before_the_stop_indicator() {
        let stop_x = 180.0;
        let painted_track_end = inactive_track_end(stop_x) + TRACK_RADIUS;
        let stop_indicator_left = stop_x - STOP_INDICATOR_RADIUS;

        assert_eq!(stop_indicator_left - painted_track_end, STOP_TRACK_GAP);
    }
}
