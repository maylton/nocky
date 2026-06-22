use gtk::{gdk, glib, prelude::*};
use std::{
    cell::{Cell, RefCell},
    f64::consts::PI,
    rc::Rc,
    time::Duration,
};

// More bands and less spatial averaging give the visualizer the lively,
// fine-grained motion seen in Noctalia without turning quiet passages into a wall.
const DISPLAY_BANDS: usize = 48;
const HALF_BANDS: usize = DISPLAY_BANDS / 2;
const SMOOTHING_TAU_MS: f64 = 60.0;
const NOISE_FLOOR: f64 = 0.028;
const TARGET_CEILING: f64 = 0.94;

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
            glib::timeout_add_local(Duration::from_millis(16), move || {
                let frame_ms = 16.0;
                let alpha = 1.0 - (-frame_ms / SMOOTHING_TAU_MS).exp();
                let target = state.target.borrow();
                let mut display = state.display.borrow_mut();
                let mut changed = false;

                for (current, target) in display.iter_mut().zip(target.iter()) {
                    let delta = *target - *current;
                    if delta.abs() < 1.0 / 2048.0 {
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
        if values.is_empty() {
            self.clear();
            return;
        }

        let mut reduced = [0.0_f64; HALF_BANDS];
        for (band, output) in reduced.iter_mut().enumerate() {
            let start = band * values.len() / HALF_BANDS;
            let mut end = (band + 1) * values.len() / HALF_BANDS;
            if end <= start {
                end = (start + 1).min(values.len());
            }
            let slice = &values[start.min(values.len() - 1)..end.max(start + 1).min(values.len())];

            // A peak/RMS blend reacts to transients like Noctalia while RMS keeps it fluid.
            let mut peak = 0.0_f64;
            let mut sum_sq = 0.0_f64;
            for value in slice {
                let value = (*value as f64).clamp(0.0, 1.0);
                peak = peak.max(value);
                sum_sq += value * value;
            }
            let rms = (sum_sq / slice.len() as f64).sqrt();
            let raw = rms * 0.72 + peak * 0.28;
            let gated = ((raw - NOISE_FLOOR) / (1.0 - NOISE_FLOOR)).clamp(0.0, 1.0);
            *output = (gated * 0.96).powf(0.86).min(TARGET_CEILING);
        }

        // Keep only a light neighbour blend. The old 22/56/22 mix made the whole
        // waveform move as a single blob instead of showing Noctalia-like detail.
        let mut shaped = [0.0_f64; HALF_BANDS];
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
            shaped[index] = (prev * 0.10 + current * 0.80 + next * 0.10).clamp(0.0, 1.0);
        }

        let mut target = self.state.target.borrow_mut();
        for (index, value) in shaped.iter().enumerate() {
            target[HALF_BANDS - 1 - index] = *value;
            target[HALF_BANDS + index] = *value;
        }
    }

    pub fn clear(&self) {
        self.state.target.borrow_mut().fill(0.0);
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
    let gap = 1.35;
    let available = (width - horizontal_padding * 2.0).max(1.0);
    let bar_width = ((available - gap * (values.len().saturating_sub(1)) as f64)
        / values.len() as f64)
        .max(1.7);
    let center_y = height / 2.0;
    let maximum_height = ((height - 8.0) * 0.96).max(8.0);

    context.set_line_width(1.0);
    context.set_source_rgba(
        accent.red() as f64,
        accent.green() as f64,
        accent.blue() as f64,
        0.12,
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
        let edge_taper = (1.0 - distance_from_center).powf(0.38);
        let tapered_value = (value * (0.84 + edge_taper * 0.16)).clamp(0.0, 1.0);
        let bar_height = (1.5 + tapered_value * maximum_height).min(maximum_height);
        let y = center_y - bar_height / 2.0;

        let highlight = 0.14 + tapered_value * 0.28;
        let red = lerp(accent.red() as f64, text.red() as f64, highlight);
        let green = lerp(accent.green() as f64, text.green() as f64, highlight);
        let blue = lerp(accent.blue() as f64, text.blue() as f64, highlight);

        rounded_rectangle(
            context,
            x,
            y,
            bar_width,
            bar_height,
            (bar_width / 2.0).min(2.4),
        );
        context.set_source_rgba(red, green, blue, 0.58 + tapered_value * 0.42);
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
