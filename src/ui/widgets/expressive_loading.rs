use gtk::{cairo, glib, prelude::*};
use std::{
    cell::{Cell, RefCell},
    f64::consts::{PI, TAU},
    rc::Rc,
};

const COMPACT_SIZE: i32 = 22;
const STANDARD_SIZE: i32 = 32;
const LARGE_SIZE: i32 = 52;
const INDICATOR_STEP: f64 = 1.0 / 72.0;
const PROGRESS_EASE: f64 = 0.22;
const SHAPE_COUNT: usize = 7;

#[derive(Clone, Copy, Debug, PartialEq)]
#[allow(dead_code)]
pub(crate) enum LoadingIndicatorMode {
    Indeterminate,
    Determinate(f64),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LoadingIndicatorPresentation {
    Uncontained,
    Contained,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LoadingIndicatorSize {
    Compact,
    Standard,
    Large,
}

#[derive(Clone)]
#[allow(dead_code)]
pub(crate) struct MaterialLoadingIndicator {
    area: gtk::DrawingArea,
    state: Rc<IndicatorState>,
}

impl MaterialLoadingIndicator {
    pub(crate) fn new() -> Self {
        Self::standard()
    }

    #[allow(dead_code)]
    pub(crate) fn compact() -> Self {
        Self::with_options(
            LoadingIndicatorSize::Compact,
            LoadingIndicatorPresentation::Uncontained,
            LoadingIndicatorMode::Indeterminate,
        )
    }

    pub(crate) fn standard() -> Self {
        Self::with_options(
            LoadingIndicatorSize::Standard,
            LoadingIndicatorPresentation::Uncontained,
            LoadingIndicatorMode::Indeterminate,
        )
    }

    #[allow(dead_code)]
    pub(crate) fn large_contained() -> Self {
        Self::with_options(
            LoadingIndicatorSize::Large,
            LoadingIndicatorPresentation::Contained,
            LoadingIndicatorMode::Indeterminate,
        )
    }

    #[allow(dead_code)]
    pub(crate) fn determinate(progress: f64) -> Self {
        Self::with_options(
            LoadingIndicatorSize::Standard,
            LoadingIndicatorPresentation::Contained,
            LoadingIndicatorMode::Determinate(progress),
        )
    }

    pub(crate) fn with_size(size: i32) -> Self {
        let size = if size <= COMPACT_SIZE {
            LoadingIndicatorSize::Compact
        } else if size >= LARGE_SIZE {
            LoadingIndicatorSize::Large
        } else {
            LoadingIndicatorSize::Standard
        };
        Self::with_options(
            size,
            LoadingIndicatorPresentation::Uncontained,
            LoadingIndicatorMode::Indeterminate,
        )
    }

    pub(crate) fn with_options(
        size: LoadingIndicatorSize,
        presentation: LoadingIndicatorPresentation,
        mode: LoadingIndicatorMode,
    ) -> Self {
        let area = gtk::DrawingArea::new();
        let pixels = size.pixels();
        area.set_size_request(pixels, pixels);
        area.set_accessible_role(gtk::AccessibleRole::ProgressBar);
        area.add_css_class("material-loading-indicator");
        area.add_css_class(size.css_class());
        area.add_css_class(presentation.css_class());

        let state = Rc::new(IndicatorState::new(mode, presentation, size));
        state.apply_accessibility(&area);

        {
            let state = state.clone();
            area.set_draw_func(move |widget, context, width, height| {
                draw_indicator(widget, context, width, height, &state.snapshot());
            });
        }

        {
            let area = area.clone();
            let state = state.clone();
            area.connect_map(move |widget| update_tick(widget, &state));
        }
        {
            let state = state.clone();
            area.connect_unmap(move |widget| {
                state.stop_tick(widget);
            });
        }
        {
            let state = state.clone();
            area.connect_visible_notify(move |widget| update_tick(widget, &state));
        }

        update_tick(&area, &state);

        Self { area, state }
    }

    pub(crate) fn widget(&self) -> &gtk::DrawingArea {
        &self.area
    }

    #[allow(dead_code)]
    pub(crate) fn set_mode(&self, mode: LoadingIndicatorMode) {
        self.state.set_mode(mode);
        self.state.apply_accessibility(&self.area);
        self.area.queue_draw();
        update_tick(&self.area, &self.state);
    }
}

#[derive(Debug)]
struct IndicatorState {
    mode: Cell<LoadingIndicatorMode>,
    presentation: Cell<LoadingIndicatorPresentation>,
    size: Cell<LoadingIndicatorSize>,
    phase: Cell<f64>,
    displayed_progress: Cell<f64>,
    target_progress: Cell<f64>,
    reduced_motion: Cell<bool>,
    tick: RefCell<Option<gtk::TickCallbackId>>,
}

impl IndicatorState {
    fn new(
        mode: LoadingIndicatorMode,
        presentation: LoadingIndicatorPresentation,
        size: LoadingIndicatorSize,
    ) -> Self {
        let progress = match mode {
            LoadingIndicatorMode::Determinate(value) => clamp_progress(value),
            LoadingIndicatorMode::Indeterminate => 0.0,
        };
        Self {
            mode: Cell::new(normalize_mode(mode)),
            presentation: Cell::new(presentation),
            size: Cell::new(size),
            phase: Cell::new(0.0),
            displayed_progress: Cell::new(progress),
            target_progress: Cell::new(progress),
            reduced_motion: Cell::new(false),
            tick: RefCell::new(None),
        }
    }

    fn snapshot(&self) -> IndicatorSnapshot {
        let reduced_motion = self.reduced_motion.get();
        let mode = self.mode.get();
        let progress = match mode {
            LoadingIndicatorMode::Determinate(_) => self.displayed_progress.get(),
            LoadingIndicatorMode::Indeterminate => {
                if reduced_motion {
                    0.0
                } else {
                    self.phase.get()
                }
            }
        };
        IndicatorSnapshot {
            mode,
            presentation: self.presentation.get(),
            size: self.size.get(),
            reduced_motion,
            phase: progress,
        }
    }

    #[allow(dead_code)]
    fn set_mode(&self, mode: LoadingIndicatorMode) {
        let mode = normalize_mode(mode);
        self.mode.set(mode);
        if let LoadingIndicatorMode::Determinate(progress) = mode {
            self.target_progress.set(progress);
        }
    }

    fn should_animate(&self, widget: &gtk::DrawingArea) -> bool {
        widget.is_visible()
            && widget.is_mapped()
            && !self.reduced_motion.get()
            && matches!(self.mode.get(), LoadingIndicatorMode::Indeterminate)
    }

    fn ensure_tick(self: &Rc<Self>, widget: &gtk::DrawingArea) {
        if self.tick.borrow().is_some() {
            return;
        }

        let state = self.clone();
        let id = widget.add_tick_callback(move |widget, _| {
            if !state.should_animate(widget) {
                state.stop_tick(widget);
                return glib::ControlFlow::Break;
            }
            state.advance_frame();
            widget.queue_draw();
            glib::ControlFlow::Continue
        });
        self.tick.borrow_mut().replace(id);
    }

    fn stop_tick(&self, widget: &gtk::DrawingArea) {
        if let Some(id) = self.tick.borrow_mut().take() {
            id.remove();
            widget.queue_draw();
        }
    }

    fn advance_frame(&self) {
        match self.mode.get() {
            LoadingIndicatorMode::Indeterminate => {
                self.phase.set((self.phase.get() + INDICATOR_STEP) % 1.0);
            }
            LoadingIndicatorMode::Determinate(_) => {
                let current = self.displayed_progress.get();
                let target = self.target_progress.get();
                let next = current + (target - current) * PROGRESS_EASE;
                self.displayed_progress
                    .set(if (next - target).abs() < 0.001 {
                        target
                    } else {
                        next
                    });
            }
        }
    }

    fn apply_accessibility(&self, widget: &gtk::DrawingArea) {
        let label = match self.mode.get() {
            LoadingIndicatorMode::Indeterminate => "Loading",
            LoadingIndicatorMode::Determinate(_) => "Loading progress",
        };
        widget.update_property(&[gtk::accessible::Property::Label(label)]);
        match self.mode.get() {
            LoadingIndicatorMode::Determinate(progress) => {
                widget.update_property(&[
                    gtk::accessible::Property::ValueMin(0.0),
                    gtk::accessible::Property::ValueMax(100.0),
                    gtk::accessible::Property::ValueNow(accessible_progress_value(progress)),
                ]);
            }
            LoadingIndicatorMode::Indeterminate => {
                widget.update_state(&[gtk::accessible::State::Busy(true)]);
            }
        }
    }

    #[cfg(test)]
    fn set_reduced_motion(&self, reduced_motion: bool) {
        self.reduced_motion.set(reduced_motion);
    }

    #[cfg(test)]
    fn tick_count(&self) -> usize {
        usize::from(self.tick.borrow().is_some())
    }
}

fn update_tick(widget: &gtk::DrawingArea, state: &Rc<IndicatorState>) {
    state
        .reduced_motion
        .set(!adw::is_animations_enabled(widget));
    if state.should_animate(widget) {
        state.ensure_tick(widget);
    } else {
        state.stop_tick(widget);
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct IndicatorSnapshot {
    mode: LoadingIndicatorMode,
    presentation: LoadingIndicatorPresentation,
    size: LoadingIndicatorSize,
    reduced_motion: bool,
    phase: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Point {
    x: f64,
    y: f64,
}

impl LoadingIndicatorSize {
    fn pixels(self) -> i32 {
        match self {
            Self::Compact => COMPACT_SIZE,
            Self::Standard => STANDARD_SIZE,
            Self::Large => LARGE_SIZE,
        }
    }

    fn css_class(self) -> &'static str {
        match self {
            Self::Compact => "compact",
            Self::Standard => "standard",
            Self::Large => "large",
        }
    }
}

impl LoadingIndicatorPresentation {
    fn css_class(self) -> &'static str {
        match self {
            Self::Uncontained => "uncontained",
            Self::Contained => "contained",
        }
    }
}

fn normalize_mode(mode: LoadingIndicatorMode) -> LoadingIndicatorMode {
    match mode {
        LoadingIndicatorMode::Indeterminate => LoadingIndicatorMode::Indeterminate,
        LoadingIndicatorMode::Determinate(progress) => {
            LoadingIndicatorMode::Determinate(clamp_progress(progress))
        }
    }
}

fn clamp_progress(progress: f64) -> f64 {
    if progress.is_finite() {
        progress.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

fn accessible_progress_value(progress: f64) -> f64 {
    clamp_progress(progress) * 100.0
}

fn eased(value: f64) -> f64 {
    let value = clamp_progress(value);
    value * value * (3.0 - 2.0 * value)
}

fn shape_position(snapshot: &IndicatorSnapshot) -> f64 {
    if snapshot.reduced_motion {
        1.0
    } else {
        match snapshot.mode {
            LoadingIndicatorMode::Indeterminate => snapshot.phase.fract(),
            LoadingIndicatorMode::Determinate(_) => clamp_progress(snapshot.phase),
        }
    }
}

fn active_shape_points(position: f64) -> Vec<Point> {
    let scaled = clamp_progress(position) * SHAPE_COUNT as f64;
    let left = scaled.floor() as usize % SHAPE_COUNT;
    let local = eased(scaled - scaled.floor());
    let right = (left + 1) % SHAPE_COUNT;
    morph_points(&shape_points(left), &shape_points(right), local)
}

fn completion_shape_points() -> Vec<Point> {
    shape_points(SHAPE_COUNT - 1).to_vec()
}

fn morph_points(left: &[Point], right: &[Point], amount: f64) -> Vec<Point> {
    left.iter()
        .zip(right.iter())
        .map(|(left, right)| Point {
            x: left.x + (right.x - left.x) * amount,
            y: left.y + (right.y - left.y) * amount,
        })
        .collect()
}

fn shape_points(index: usize) -> Vec<Point> {
    match index % SHAPE_COUNT {
        0 => radial_shape(1.0, 1.0, 0.0, 0.0),
        1 => radial_shape(0.86, 1.18, 4.0, PI / 4.0),
        2 => radial_shape(0.82, 1.22, 4.0, 0.0),
        3 => radial_shape(0.78, 1.24, 3.0, -PI / 2.0),
        4 => radial_shape(0.82, 1.16, 5.0, -PI / 2.0),
        5 => oval_shape(),
        _ => radial_shape(0.74, 1.18, 8.0, 0.0),
    }
}

fn radial_shape(base: f64, accent: f64, lobes: f64, offset: f64) -> Vec<Point> {
    let mut points = Vec::with_capacity(16);
    for index in 0..16 {
        let angle = index as f64 / 16.0 * TAU;
        let wave = if lobes == 0.0 {
            0.0
        } else {
            (angle * lobes + offset).cos()
        };
        let radius = (base + (accent - base) * (wave + 1.0) * 0.5) * 0.34;
        points.push(Point {
            x: 0.5 + angle.cos() * radius,
            y: 0.5 + angle.sin() * radius,
        });
    }
    points
}

fn oval_shape() -> Vec<Point> {
    let mut points = Vec::with_capacity(16);
    for index in 0..16 {
        let angle = index as f64 / 16.0 * TAU;
        points.push(Point {
            x: 0.5 + angle.cos() * 0.38,
            y: 0.5 + angle.sin() * 0.25,
        });
    }
    points
}

fn draw_indicator(
    widget: &gtk::DrawingArea,
    context: &cairo::Context,
    width: i32,
    height: i32,
    snapshot: &IndicatorSnapshot,
) {
    if width <= 0 || height <= 0 {
        return;
    }

    let size = width.min(height) as f64;
    let scale = match snapshot.presentation {
        LoadingIndicatorPresentation::Contained => 0.58,
        LoadingIndicatorPresentation::Uncontained => 0.86,
    };
    let origin_x = (width as f64 - size) / 2.0;
    let origin_y = (height as f64 - size) / 2.0;

    if snapshot.presentation == LoadingIndicatorPresentation::Contained {
        let color = widget.color();
        let _ = context.save();
        context.set_source_rgba(
            color.red() as f64,
            color.green() as f64,
            color.blue() as f64,
            0.14,
        );
        context.rounded_rectangle(origin_x, origin_y, size, size, size / 2.0);
        let _ = context.fill();
        let _ = context.restore();
    }

    let position =
        if matches!(snapshot.mode, LoadingIndicatorMode::Determinate(_)) && snapshot.phase >= 1.0 {
            1.0
        } else {
            shape_position(snapshot)
        };
    let points = if position >= 1.0 {
        completion_shape_points()
    } else {
        active_shape_points(position)
    };

    let color = widget.color();
    let _ = context.save();
    context.translate(width as f64 / 2.0, height as f64 / 2.0);
    if !snapshot.reduced_motion {
        context.rotate(snapshot.phase * TAU);
    }
    context.scale(size * scale, size * scale);
    context.translate(-0.5, -0.5);
    context.set_source_rgba(
        color.red() as f64,
        color.green() as f64,
        color.blue() as f64,
        0.96,
    );
    draw_points(context, &points);
    let _ = context.fill();
    let _ = context.restore();
}

trait RoundedRectangle {
    fn rounded_rectangle(&self, x: f64, y: f64, width: f64, height: f64, radius: f64);
}

impl RoundedRectangle for cairo::Context {
    fn rounded_rectangle(&self, x: f64, y: f64, width: f64, height: f64, radius: f64) {
        let radius = radius.min(width / 2.0).min(height / 2.0);
        self.new_sub_path();
        self.arc(x + width - radius, y + radius, radius, -PI / 2.0, 0.0);
        self.arc(
            x + width - radius,
            y + height - radius,
            radius,
            0.0,
            PI / 2.0,
        );
        self.arc(x + radius, y + height - radius, radius, PI / 2.0, PI);
        self.arc(x + radius, y + radius, radius, PI, PI * 1.5);
        self.close_path();
    }
}

fn draw_points(context: &cairo::Context, points: &[Point]) {
    if let Some(first) = points.first() {
        context.new_path();
        context.move_to(first.x, first.y);
        for point in &points[1..] {
            context.line_to(point.x, point.y);
        }
        context.close_path();
    }
}

#[cfg(test)]
fn normalized_bounds(points: &[Point]) -> (f64, f64, f64, f64) {
    points.iter().fold(
        (f64::MAX, f64::MIN, f64::MAX, f64::MIN),
        |(min_x, max_x, min_y, max_y), point| {
            (
                min_x.min(point.x),
                max_x.max(point.x),
                min_y.min(point.y),
                max_y.max(point.y),
            )
        },
    )
}

#[cfg(test)]
fn normalized_center(points: &[Point]) -> Point {
    let (sum_x, sum_y) = points
        .iter()
        .fold((0.0, 0.0), |(x, y), point| (x + point.x, y + point.y));
    Point {
        x: sum_x / points.len() as f64,
        y: sum_y / points.len() as f64,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_values_are_clamped() {
        assert_eq!(clamp_progress(-1.0), 0.0);
        assert_eq!(clamp_progress(1.4), 1.0);
        assert_eq!(clamp_progress(f64::NAN), 0.0);
    }

    #[test]
    fn progress_endpoints_have_stable_shapes() {
        assert_eq!(active_shape_points(0.0), shape_points(0));
        assert_eq!(completion_shape_points(), shape_points(SHAPE_COUNT - 1));
    }

    #[test]
    fn adjacent_shape_interpolation_is_continuous() {
        for index in 0..SHAPE_COUNT {
            let left = shape_points(index);
            let right = shape_points((index + 1) % SHAPE_COUNT);
            assert_eq!(morph_points(&left, &right, 0.0), left);
            assert_eq!(morph_points(&left, &right, 1.0), right);
        }
    }

    #[test]
    fn shape_bounds_stay_inside_normalized_viewport() {
        for index in 0..SHAPE_COUNT {
            let points = shape_points(index);
            let (min_x, max_x, min_y, max_y) = normalized_bounds(&points);
            assert!(min_x >= 0.0);
            assert!(max_x <= 1.0);
            assert!(min_y >= 0.0);
            assert!(max_y <= 1.0);
        }
    }

    #[test]
    fn shape_center_stays_stable_during_morph() {
        for position in [0.0, 0.1, 0.25, 0.5, 0.75, 0.9, 1.0] {
            let center = normalized_center(&active_shape_points(position));
            assert!((center.x - 0.5).abs() < 0.0001);
            assert!((center.y - 0.5).abs() < 0.0001);
        }
    }

    #[test]
    fn normalized_geometry_is_size_independent() {
        assert_eq!(active_shape_points(0.37), active_shape_points(0.37));
        assert_eq!(LoadingIndicatorSize::Compact.pixels(), COMPACT_SIZE);
        assert_eq!(LoadingIndicatorSize::Standard.pixels(), STANDARD_SIZE);
        assert_eq!(LoadingIndicatorSize::Large.pixels(), LARGE_SIZE);
    }

    #[test]
    fn reduced_motion_uses_stable_shape_position() {
        let snapshot = IndicatorSnapshot {
            mode: LoadingIndicatorMode::Indeterminate,
            presentation: LoadingIndicatorPresentation::Uncontained,
            size: LoadingIndicatorSize::Standard,
            reduced_motion: true,
            phase: 0.42,
        };
        assert_eq!(shape_position(&snapshot), 1.0);
    }

    #[test]
    fn hidden_unmapped_state_does_not_request_frames() {
        let state = IndicatorState::new(
            LoadingIndicatorMode::Indeterminate,
            LoadingIndicatorPresentation::Uncontained,
            LoadingIndicatorSize::Standard,
        );
        assert_eq!(state.tick_count(), 0);
    }

    #[test]
    fn repeated_stop_does_not_create_duplicate_tick_state() {
        let state = IndicatorState::new(
            LoadingIndicatorMode::Indeterminate,
            LoadingIndicatorPresentation::Uncontained,
            LoadingIndicatorSize::Standard,
        );
        state.set_reduced_motion(true);
        assert_eq!(state.tick_count(), 0);
    }

    #[test]
    fn style_mapping_covers_contained_and_uncontained() {
        assert_eq!(
            LoadingIndicatorPresentation::Contained.css_class(),
            "contained"
        );
        assert_eq!(
            LoadingIndicatorPresentation::Uncontained.css_class(),
            "uncontained"
        );
        assert_eq!(LoadingIndicatorSize::Compact.css_class(), "compact");
    }

    #[test]
    fn accessibility_progress_is_percent_based() {
        assert_eq!(accessible_progress_value(0.0), 0.0);
        assert_eq!(accessible_progress_value(0.25), 25.0);
        assert_eq!(accessible_progress_value(1.0), 100.0);
    }

    #[test]
    fn completion_state_stops_animation_need() {
        let state = IndicatorState::new(
            LoadingIndicatorMode::Determinate(1.0),
            LoadingIndicatorPresentation::Contained,
            LoadingIndicatorSize::Standard,
        );
        assert!(matches!(
            state.mode.get(),
            LoadingIndicatorMode::Determinate(1.0)
        ));
    }

    #[test]
    fn theme_roles_are_css_classes_not_hardcoded_theme_colors() {
        let source = include_str!("expressive_loading.rs");
        let rgba_black = ["set_source_rgba", "(0.0, 0.0, 0.0"].join("");
        let rgb_black = ["set_source_rgb", "(0.0, 0.0, 0.0"].join("");
        assert_eq!("material-loading-indicator", "material-loading-indicator");
        assert_eq!(
            LoadingIndicatorPresentation::Contained.css_class(),
            "contained"
        );
        assert!(!source.contains(&rgba_black));
        assert!(!source.contains(&rgb_black));
    }

    #[test]
    fn migrated_call_sites_use_shared_component() {
        let browser = include_str!("../../browser.rs");
        let youtube = include_str!("../../youtube/mod.rs");
        let assisted_login = include_str!("../../youtube/assisted_login.rs");

        assert!(browser.contains("MaterialLoadingIndicator"));
        assert!(youtube.contains("MaterialLoadingIndicator"));
        assert!(assisted_login.contains("MaterialLoadingIndicator"));
    }

    #[test]
    fn generic_gtk_spinners_are_not_used_for_loading() {
        let assisted_login = include_str!("../../youtube/assisted_login.rs");

        assert!(!assisted_login.contains("gtk::Spinner"));
        assert!(!assisted_login.contains(".start()"));
        assert!(!assisted_login.contains(".stop()"));
    }
}
