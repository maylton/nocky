use gtk::{gdk, glib, prelude::*};
use std::{
    cell::{Cell, RefCell},
    f64::consts::PI,
    rc::Rc,
    time::Duration,
};

const DISPLAY_BANDS: usize = 32;
const HALF_BANDS: usize = DISPLAY_BANDS / 2;
const ATTACK_TAU_MS: f64 = 46.0;
const RELEASE_TAU_MS: f64 = 112.0;

#[derive(Clone)]
pub struct SpectrumVisualizer {
    root: gtk::DrawingArea,
    state: Rc<VisualizerState>,
}

struct VisualizerState {
    target: RefCell<Vec<f64>>,
    display: RefCell<Vec<f64>>,
    active: Cell<bool>,
}

impl SpectrumVisualizer {
    pub fn new() -> Self {
        let root = gtk::DrawingArea::new();
        root.set_hexpand(true);
        root.set_height_request(64);
        root.set_content_height(64);
        root.add_css_class("audio-visualizer");
        root.set_tooltip_text(Some("Espectro de áudio"));

        let state = Rc::new(VisualizerState {
            target: RefCell::new(vec![0.0; DISPLAY_BANDS]),
            display: RefCell::new(vec![0.0; DISPLAY_BANDS]),
            active: Cell::new(false),
        });

        {
            let state = state.clone();
            root.set_draw_func(move |widget, context, width, height| {
                draw_spectrum(widget, context, width, height, &state.display.borrow());
            });
        }

        {
            let state = state.clone();
            let drawing = root.clone();
            glib::timeout_add_local(Duration::from_millis(33), move || {
                if !state.active.get() || !drawing.is_mapped() {
                    return glib::ControlFlow::Continue;
                }

                let frame_ms = 33.0;
                let target = state.target.borrow();
                let mut display = state.display.borrow_mut();
                let mut changed = false;

                for (current, target) in display.iter_mut().zip(target.iter()) {
                    let tau = if *target > *current {
                        ATTACK_TAU_MS
                    } else {
                        RELEASE_TAU_MS
                    };
                    let alpha = 1.0 - (-frame_ms / tau).exp();
                    let delta = *target - *current;
                    if delta.abs() < 1.0 / 1024.0 {
                        if *current != *target {
                            *current = *target;
                            changed = true;
                        }
                    } else {
                        *current += delta * alpha;
                        changed = true;
                    }
                }

                if changed {
                    drawing.queue_draw();
                }
                glib::ControlFlow::Continue
            });
        }

        Self { root, state }
    }

    pub fn widget(&self) -> &gtk::DrawingArea {
        &self.root
    }

    pub fn set_active(&self, active: bool) {
        self.state.active.set(active);
        if !active {
            self.clear();
        }
    }

    pub fn set_values(&self, values: &[f32]) {
        if !self.state.active.get() {
            return;
        }

        let mut reduced = [0.0_f64; HALF_BANDS];
        if values.is_empty() {
            self.clear();
            return;
        }

        for (band, output) in reduced.iter_mut().enumerate() {
            let start = band * values.len() / HALF_BANDS;
            let mut end = (band + 1) * values.len() / HALF_BANDS;
            if end <= start {
                end = (start + 1).min(values.len());
            }
            let slice = &values[start.min(values.len() - 1)..end.max(start + 1).min(values.len())];
            let average = slice.iter().map(|value| *value as f64).sum::<f64>() / slice.len() as f64;
            *output = average.clamp(0.0, 1.0);
        }

        let mut smoothed = [0.0_f64; HALF_BANDS];
        for index in 0..HALF_BANDS {
            let current = reduced[index];
            let prev = if index > 0 {
                reduced[index - 1]
            } else {
                current
            };
            let next = if index + 1 < HALF_BANDS {
                reduced[index + 1]
            } else {
                current
            };
            smoothed[index] = (prev * 0.22 + current * 0.56 + next * 0.22).clamp(0.0, 1.0);
        }

        let mut target = self.state.target.borrow_mut();
        for (index, value) in smoothed.iter().enumerate() {
            let shaped = (value * 0.92).powf(1.08).min(0.84);
            target[HALF_BANDS - 1 - index] = shaped;
            target[HALF_BANDS + index] = shaped;
        }
    }

    pub fn clear(&self) {
        self.state.target.borrow_mut().fill(0.0);
        self.state.display.borrow_mut().fill(0.0);
        self.root.queue_draw();
    }
}

#[allow(deprecated)]
fn draw_spectrum(
    widget: &gtk::DrawingArea,
    context: &gtk::cairo::Context,
    width: i32,
    height: i32,
    values: &[f64],
) {
    if width <= 0 || height <= 0 || values.is_empty() {
        return;
    }

    let style = widget.style_context();
    let accent = style
        .lookup_color("accent_color")
        .unwrap_or_else(|| gdk::RGBA::new(0.34, 0.82, 0.96, 1.0));
    let text = style
        .lookup_color("window_fg_color")
        .unwrap_or_else(|| gdk::RGBA::new(0.95, 0.97, 1.0, 1.0));

    let width = width as f64;
    let height = height as f64;
    let horizontal_padding = 8.0;
    let gap = 1.8;
    let available = (width - horizontal_padding * 2.0).max(1.0);
    let bar_width = ((available - gap * (values.len().saturating_sub(1)) as f64)
        / values.len() as f64)
        .max(2.2);
    let center_y = height / 2.0;
    let maximum_height = ((height - 10.0) * 0.88).max(8.0);

    context.set_line_width(1.0);
    context.set_source_rgba(
        accent.red() as f64,
        accent.green() as f64,
        accent.blue() as f64,
        0.16,
    );
    context.move_to(horizontal_padding, center_y);
    context.line_to(width - horizontal_padding, center_y);
    let _ = context.stroke();

    for (index, value) in values.iter().enumerate() {
        let progress = if values.len() <= 1 {
            0.5
        } else {
            index as f64 / (values.len() - 1) as f64
        };
        let x = horizontal_padding + index as f64 * (bar_width + gap);
        let distance_from_center = (progress - 0.5).abs() * 2.0;
        let edge_taper = (1.0 - distance_from_center).powf(0.28);
        let tapered_value = (value * (0.78 + edge_taper * 0.22)).clamp(0.0, 1.0);
        let bar_height = (2.0 + tapered_value * maximum_height).min(maximum_height);
        let y = center_y - bar_height / 2.0;
        let glow_height = (bar_height + 4.0).min(maximum_height + 4.0);
        let glow_y = center_y - glow_height / 2.0;

        let highlight = 0.18 + tapered_value * 0.32;
        let red = lerp(accent.red() as f64, text.red() as f64, highlight);
        let green = lerp(accent.green() as f64, text.green() as f64, highlight);
        let blue = lerp(accent.blue() as f64, text.blue() as f64, highlight);

        rounded_rectangle(
            context,
            x - 0.5,
            glow_y,
            bar_width + 1.0,
            glow_height,
            ((bar_width + 1.0) / 2.0).min(3.2),
        );
        context.set_source_rgba(red, green, blue, 0.09 + tapered_value * 0.11);
        let _ = context.fill();

        rounded_rectangle(
            context,
            x,
            y,
            bar_width,
            bar_height,
            (bar_width / 2.0).min(2.8),
        );
        context.set_source_rgba(red, green, blue, 0.52 + tapered_value * 0.48);
        let _ = context.fill();
    }
}

fn rounded_rectangle(
    context: &gtk::cairo::Context,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    radius: f64,
) {
    let radius = radius.min(width / 2.0).min(height / 2.0);
    context.new_sub_path();
    context.arc(x + width - radius, y + radius, radius, -PI / 2.0, 0.0);
    context.arc(
        x + width - radius,
        y + height - radius,
        radius,
        0.0,
        PI / 2.0,
    );
    context.arc(x + radius, y + height - radius, radius, PI / 2.0, PI);
    context.arc(x + radius, y + radius, radius, PI, PI * 1.5);
    context.close_path();
}

fn lerp(start: f64, end: f64, amount: f64) -> f64 {
    start + (end - start) * amount
}
