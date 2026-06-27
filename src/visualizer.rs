use gtk::{glib, prelude::*};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::{Duration, Instant},
};

const ANALYSIS_BANDS: usize = 32;
const VIEW_BANDS: usize = 16;
const DISPLAY_BANDS: usize = VIEW_BANDS * 2;

const FRAME_INTERVAL_MS: u64 = 16;
const FRAME_RATE_HZ: usize = 60;
const SMOOTHING_TAU_MS: f64 = 60.0;

const FFT_SIZE: f64 = 4096.0;
const FFT_REFERENCE_MAGNITUDE: f64 = FFT_SIZE / 4.0;
const NOISE_REDUCTION: f64 = 0.77;
const MAX_BAND_LEVEL: f64 = 0.9;
const MIN_SENSITIVITY: f64 = 0.001;
const MAX_SENSITIVITY: f64 = 30.0;
const FALL_STEP: f64 = 0.028;
const MONSTERCAT_FACTOR: f64 = 1.5;
const MIN_SPREAD: f64 = 0.001;
const GAP_TO_BAR_RATIO: f64 = 0.5;

#[derive(Clone)]
pub struct SpectrumVisualizer {
    root: gtk::DrawingArea,
    state: Rc<VisualizerState>,
}

struct VisualizerState {
    target: RefCell<Vec<f64>>,
    display: RefCell<Vec<f64>>,
    processor: RefCell<NoctaliaSpectrumProcessor>,
    active: Cell<bool>,
}

struct NoctaliaSpectrumProcessor {
    previous: [f64; VIEW_BANDS],
    peak: [f64; VIEW_BANDS],
    fall: [f64; VIEW_BANDS],
    memory: [f64; VIEW_BANDS],
    sensitivity: f64,
    sensitivity_initializing: bool,
    idle_frames: usize,
}

impl Default for NoctaliaSpectrumProcessor {
    fn default() -> Self {
        Self {
            previous: [0.0; VIEW_BANDS],
            peak: [0.0; VIEW_BANDS],
            fall: [0.0; VIEW_BANDS],
            memory: [0.0; VIEW_BANDS],
            sensitivity: 0.01,
            sensitivity_initializing: true,
            idle_frames: 0,
        }
    }
}

impl NoctaliaSpectrumProcessor {
    fn reset(&mut self) {
        *self = Self::default();
    }

    fn process(&mut self, decibels: &[f32]) -> [f64; VIEW_BANDS] {
        if decibels.is_empty() {
            return [0.0; VIEW_BANDS];
        }

        let mut analysis = self.analysis_magnitudes(decibels);
        let noise_gate = NOISE_REDUCTION * FFT_SIZE * 0.00005;

        for band in &mut analysis {
            *band = (*band - noise_gate).max(0.0).ln_1p() * self.sensitivity;
        }

        let mut overshoot = false;
        let mut silence = true;

        for band in &mut analysis {
            if *band > MAX_BAND_LEVEL {
                overshoot = true;
                *band = MAX_BAND_LEVEL;
            }

            if *band > 0.01 {
                silence = false;
            }
        }

        if overshoot {
            self.sensitivity *= 0.98;
            self.sensitivity_initializing = false;
        } else if !silence {
            self.sensitivity *= 1.001;
            if self.sensitivity_initializing {
                self.sensitivity *= 1.1;
            }
        }

        self.sensitivity = self.sensitivity.clamp(MIN_SENSITIVITY, MAX_SENSITIVITY);

        if silence {
            self.idle_frames += 1;
        } else {
            self.idle_frames = 0;
        }

        let mut bands = [0.0; VIEW_BANDS];

        for (index, output) in bands.iter_mut().enumerate() {
            let low = index * ANALYSIS_BANDS / VIEW_BANDS;
            let high = ((index + 1) * ANALYSIS_BANDS / VIEW_BANDS)
                .saturating_sub(1)
                .max(low);

            *output = analysis[low..=high].iter().copied().fold(0.0, f64::max);
        }

        let gravity_modifier = (1.54 / NOISE_REDUCTION.max(0.01)).max(1.0);

        for (index, band) in bands.iter_mut().enumerate() {
            if *band < self.previous[index] && NOISE_REDUCTION > 0.1 {
                *band = (self.peak[index]
                    * (1.0 - self.fall[index] * self.fall[index] * gravity_modifier))
                    .max(0.0);
                self.fall[index] += FALL_STEP;
            } else {
                self.peak[index] = *band;
                self.fall[index] = 0.0;
            }

            self.previous[index] = *band;
            *band = (self.memory[index] * NOISE_REDUCTION + *band * (1.0 - NOISE_REDUCTION))
                .clamp(0.0, MAX_BAND_LEVEL);
            self.memory[index] = *band;
        }

        for center in 0..VIEW_BANDS {
            let mut spread = bands[center] / MONSTERCAT_FACTOR;
            let mut index = center;

            while index > 0 && spread > MIN_SPREAD {
                index -= 1;
                bands[index] = bands[index].max(spread);
                spread /= MONSTERCAT_FACTOR;
            }

            spread = bands[center] / MONSTERCAT_FACTOR;
            index = center + 1;

            while index < VIEW_BANDS && spread > MIN_SPREAD {
                bands[index] = bands[index].max(spread);
                spread /= MONSTERCAT_FACTOR;
                index += 1;
            }
        }

        for band in &mut bands {
            *band = band.clamp(0.0, MAX_BAND_LEVEL);
        }

        if self.idle_frames >= FRAME_RATE_HZ {
            self.previous.fill(0.0);
            self.peak.fill(0.0);
            self.fall.fill(0.0);
            self.memory.fill(0.0);
            return [0.0; VIEW_BANDS];
        }

        bands
    }

    fn analysis_magnitudes(&self, decibels: &[f32]) -> [f64; ANALYSIS_BANDS] {
        let mut analysis = [0.0; ANALYSIS_BANDS];

        for (band, output) in analysis.iter_mut().enumerate() {
            let start = band * decibels.len() / ANALYSIS_BANDS;
            let mut end = (band + 1) * decibels.len() / ANALYSIS_BANDS;

            if end <= start {
                end = (start + 1).min(decibels.len());
            }

            let start = start.min(decibels.len() - 1);
            let end = end.max(start + 1).min(decibels.len());

            let strongest_db = decibels[start..end]
                .iter()
                .copied()
                .filter(|value| value.is_finite())
                .fold(f32::NEG_INFINITY, f32::max);

            if strongest_db.is_finite() {
                let db = f64::from(strongest_db).clamp(-120.0, 0.0);
                *output = 10.0_f64.powf(db / 20.0) * FFT_REFERENCE_MAGNITUDE;
            }
        }

        analysis
    }
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
            target: RefCell::new(vec![0.0; VIEW_BANDS]),
            display: RefCell::new(vec![0.0; VIEW_BANDS]),
            processor: RefCell::new(NoctaliaSpectrumProcessor::default()),
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
            let mut last_frame = Instant::now();

            glib::timeout_add_local(Duration::from_millis(FRAME_INTERVAL_MS), move || {
                let now = Instant::now();
                let frame_ms = now
                    .duration_since(last_frame)
                    .as_secs_f64()
                    .mul_add(1000.0, 0.0)
                    .clamp(1.0, 64.0);
                last_frame = now;

                if !state.active.get() || !drawing.is_mapped() {
                    return glib::ControlFlow::Continue;
                }

                let alpha = 1.0 - (-frame_ms / SMOOTHING_TAU_MS).exp();
                let target = state.target.borrow();
                let mut display = state.display.borrow_mut();
                let mut changed = false;

                for (current, target) in display.iter_mut().zip(target.iter()) {
                    let delta = *target - *current;

                    if delta.abs() < 1.0 / 512.0 {
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

        let processed = self.state.processor.borrow_mut().process(values);
        self.state.target.borrow_mut().copy_from_slice(&processed);
    }

    pub fn clear(&self) {
        self.state.target.borrow_mut().fill(0.0);
        self.state.display.borrow_mut().fill(0.0);
        self.state.processor.borrow_mut().reset();
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
    // The theme assigns the visualizer semantic color through CSS.
    // Noctalia uses accent_color and Material uses the artwork-derived
    // m3_primary token. Reading the computed color keeps Cairo synced
    // with live palette changes without coupling this widget to a theme.
    let style = widget.style_context();
    let accent = style.color();

    let width = f64::from(width);
    let height = f64::from(height);
    let pixel_scale = f64::from(widget.scale_factor().max(1));
    let device_pixel = 1.0 / pixel_scale;
    let bar_count = DISPLAY_BANDS;
    let gap_count = bar_count.saturating_sub(1);
    let weighted_slots = bar_count as f64 + gap_count as f64 * GAP_TO_BAR_RATIO;

    let bar_thickness = (width / weighted_slots * pixel_scale).floor() / pixel_scale;
    let bar_thickness = bar_thickness.max(device_pixel);
    let gap_thickness =
        ((bar_thickness * GAP_TO_BAR_RATIO * pixel_scale).floor() / pixel_scale).max(device_pixel);
    let stride = bar_thickness + gap_thickness;
    let used = bar_thickness * bar_count as f64 + gap_thickness * gap_count as f64;
    let start_offset = ((width - used).max(0.0) * 0.5 * pixel_scale).floor() / pixel_scale;

    for index in 0..bar_count {
        let value_index = mirrored_value_index(index);
        let raw_value = values
            .get(value_index)
            .copied()
            .unwrap_or_default()
            .clamp(0.0, 1.0);

        let mut cross_pixels = (raw_value * height * pixel_scale).round().max(1.0);

        if cross_pixels > 1.0 {
            cross_pixels = (cross_pixels * 0.5).round().max(1.0) * 2.0;
        }

        let cross_size = cross_pixels / pixel_scale;
        let x = snap_to_pixel(start_offset + index as f64 * stride, pixel_scale);
        let y = snap_to_pixel((height - cross_size) * 0.5, pixel_scale);

        context.rectangle(x, y, bar_thickness, cross_size);
        context.set_source_rgba(
            f64::from(accent.red()),
            f64::from(accent.green()),
            f64::from(accent.blue()),
            f64::from(accent.alpha()),
        );
        let _ = context.fill();
    }
}

fn mirrored_value_index(index: usize) -> usize {
    if index < VIEW_BANDS {
        VIEW_BANDS - 1 - index
    } else {
        index - VIEW_BANDS
    }
}

fn snap_to_pixel(value: f64, pixel_scale: f64) -> f64 {
    (value * pixel_scale + 0.5).floor() / pixel_scale
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mirrored_indices_match_noctalia_layout() {
        let indices = (0..DISPLAY_BANDS)
            .map(mirrored_value_index)
            .collect::<Vec<_>>();

        let expected = (0..VIEW_BANDS)
            .rev()
            .chain(0..VIEW_BANDS)
            .collect::<Vec<_>>();

        assert_eq!(indices, expected);
    }

    #[test]
    fn silence_converges_to_zero() {
        let mut processor = NoctaliaSpectrumProcessor::default();
        let silence = vec![-80.0; ANALYSIS_BANDS];

        for _ in 0..FRAME_RATE_HZ {
            let values = processor.process(&silence);
            assert!(values.iter().all(|value| *value >= 0.0));
        }

        let values = processor.process(&silence);
        assert!(values.iter().all(|value| *value == 0.0));
    }
}
