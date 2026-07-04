use gtk::{cairo, glib, prelude::*};
use std::{
    cell::{Cell, RefCell},
    f64::consts::{PI, TAU},
    rc::Rc,
};

const COMPACT_SIZE: i32 = 22;
const STANDARD_SIZE: i32 = 32;
const LARGE_SIZE: i32 = 52;
const INDETERMINATE_CYCLE_SECONDS: f64 = 2.4;
const DETERMINATE_RESPONSE_SECONDS: f64 = 0.18;
const MAX_FRAME_DELTA_SECONDS: f64 = 0.1;
const PROGRESS_EPSILON: f64 = 0.001;
const INDETERMINATE_SHAPE_COUNT: usize = 7;
const POINT_COUNT: usize = 24;
const CURVE_TENSION: f64 = 0.78;

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

    #[allow(dead_code)]
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
            let state = state.clone();
            area.connect_map(move |widget| update_tick(widget, &state));
        }
        {
            let state = state.clone();
            area.connect_unmap(move |widget| state.stop_tick(widget));
        }
        {
            let state = state.clone();
            area.connect_visible_notify(move |widget| {
                state.apply_accessibility(widget);
                update_tick(widget, &state);
            });
        }
        if let Some(settings) = gtk::Settings::default() {
            let weak_area = area.downgrade();
            let weak_state = Rc::downgrade(&state);
            settings.connect_notify_local(Some("gtk-enable-animations"), move |_, _| {
                if let (Some(area), Some(state)) = (weak_area.upgrade(), weak_state.upgrade()) {
                    update_tick(&area, &state);
                }
            });
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
    last_frame_time: Cell<Option<i64>>,
    last_accessible_percent: Cell<i32>,
    last_accessible_busy: Cell<Option<bool>>,
    tick: RefCell<Option<gtk::TickCallbackId>>,
}

impl IndicatorState {
    fn new(
        mode: LoadingIndicatorMode,
        presentation: LoadingIndicatorPresentation,
        size: LoadingIndicatorSize,
    ) -> Self {
        let mode = normalize_mode(mode);
        let progress = match mode {
            LoadingIndicatorMode::Determinate(value) => value,
            LoadingIndicatorMode::Indeterminate => 0.0,
        };
        Self {
            mode: Cell::new(mode),
            presentation: Cell::new(presentation),
            size: Cell::new(size),
            phase: Cell::new(0.0),
            displayed_progress: Cell::new(progress),
            target_progress: Cell::new(progress),
            reduced_motion: Cell::new(false),
            last_frame_time: Cell::new(None),
            last_accessible_percent: Cell::new(-1),
            last_accessible_busy: Cell::new(None),
            tick: RefCell::new(None),
        }
    }

    fn snapshot(&self) -> IndicatorSnapshot {
        IndicatorSnapshot {
            mode: self.mode.get(),
            presentation: self.presentation.get(),
            size: self.size.get(),
            reduced_motion: self.reduced_motion.get(),
            phase: self.phase.get(),
            progress: self.displayed_progress.get(),
        }
    }

    #[allow(dead_code)]
    fn set_mode(&self, mode: LoadingIndicatorMode) {
        let previous = self.mode.get();
        let mode = normalize_mode(mode);
        self.mode.set(mode);
        self.last_frame_time.set(None);
        self.last_accessible_percent.set(-1);
        self.last_accessible_busy.set(None);

        match mode {
            LoadingIndicatorMode::Indeterminate => {}
            LoadingIndicatorMode::Determinate(progress) => {
                self.target_progress.set(progress);
                if !matches!(previous, LoadingIndicatorMode::Determinate(_)) {
                    self.displayed_progress.set(progress);
                }
            }
        }
    }

    fn needs_animation(&self) -> bool {
        match self.mode.get() {
            LoadingIndicatorMode::Indeterminate => true,
            LoadingIndicatorMode::Determinate(_) => {
                (self.displayed_progress.get() - self.target_progress.get()).abs()
                    > PROGRESS_EPSILON
            }
        }
    }

    fn should_animate(&self, widget: &gtk::DrawingArea) -> bool {
        widget.is_visible()
            && widget.is_mapped()
            && !self.reduced_motion.get()
            && self.needs_animation()
    }

    fn ensure_tick(self: &Rc<Self>, widget: &gtk::DrawingArea) {
        if self.tick.borrow().is_some() {
            return;
        }

        self.last_frame_time.set(None);
        let state = self.clone();
        let id = widget.add_tick_callback(move |widget, frame_clock| {
            let animations_enabled = adw::is_animations_enabled(widget);
            state.reduced_motion.set(!animations_enabled);
            if !animations_enabled || !widget.is_visible() || !widget.is_mapped() {
                state.settle_for_reduced_motion();
                state.apply_accessibility(widget);
                state.tick.borrow_mut().take();
                widget.queue_draw();
                return glib::ControlFlow::Break;
            }

            let delta = state.frame_delta_seconds(frame_clock.frame_time());
            state.advance_by(delta);
            state.apply_accessibility(widget);
            widget.queue_draw();

            if state.needs_animation() {
                glib::ControlFlow::Continue
            } else {
                state.tick.borrow_mut().take();
                glib::ControlFlow::Break
            }
        });
        self.tick.borrow_mut().replace(id);
    }

    fn stop_tick(&self, widget: &gtk::DrawingArea) {
        self.last_frame_time.set(None);
        if let Some(id) = self.tick.borrow_mut().take() {
            id.remove();
            widget.queue_draw();
        }
    }

    fn frame_delta_seconds(&self, frame_time: i64) -> f64 {
        let previous = self.last_frame_time.replace(Some(frame_time));
        previous
            .map(|previous| (frame_time - previous).max(0) as f64 / 1_000_000.0)
            .unwrap_or(0.0)
            .clamp(0.0, MAX_FRAME_DELTA_SECONDS)
    }

    fn advance_by(&self, delta_seconds: f64) {
        let delta_seconds = delta_seconds.clamp(0.0, MAX_FRAME_DELTA_SECONDS);
        match self.mode.get() {
            LoadingIndicatorMode::Indeterminate => {
                let delta_phase = delta_seconds / INDETERMINATE_CYCLE_SECONDS;
                self.phase.set((self.phase.get() + delta_phase) % 1.0);
            }
            LoadingIndicatorMode::Determinate(_) => {
                let current = self.displayed_progress.get();
                let target = self.target_progress.get();
                let response = 1.0 - (-delta_seconds / DETERMINATE_RESPONSE_SECONDS).exp();
                let next = current + (target - current) * response;
                self.displayed_progress
                    .set(if (next - target).abs() <= PROGRESS_EPSILON {
                        target
                    } else {
                        next
                    });
            }
        }
    }

    fn settle_for_reduced_motion(&self) {
        if let LoadingIndicatorMode::Determinate(_) = self.mode.get() {
            self.displayed_progress.set(self.target_progress.get());
        }
    }

    fn apply_accessibility(&self, widget: &gtk::DrawingArea) {
        let visible = widget.is_visible();
        match self.mode.get() {
            LoadingIndicatorMode::Indeterminate => {
                self.update_busy(widget, visible);
                self.last_accessible_percent.set(-1);
            }
            LoadingIndicatorMode::Determinate(_) => {
                let progress = clamp_progress(self.displayed_progress.get());
                let percent = accessible_progress_value(progress).round() as i32;
                if self.last_accessible_percent.replace(percent) != percent {
                    widget.update_property(&[
                        gtk::accessible::Property::ValueMin(0.0),
                        gtk::accessible::Property::ValueMax(100.0),
                        gtk::accessible::Property::ValueNow(percent as f64),
                    ]);
                }
                self.update_busy(widget, visible && progress < 1.0 - PROGRESS_EPSILON);
            }
        }
    }

    fn update_busy(&self, widget: &gtk::DrawingArea, busy: bool) {
        if self.last_accessible_busy.replace(Some(busy)) != Some(busy) {
            widget.update_state(&[gtk::accessible::State::Busy(busy)]);
        }
    }

    #[cfg(test)]
    fn set_reduced_motion(&self, reduced_motion: bool) {
        self.reduced_motion.set(reduced_motion);
        if reduced_motion {
            self.settle_for_reduced_motion();
        }
    }

    #[cfg(test)]
    fn tick_count(&self) -> usize {
        usize::from(self.tick.borrow().is_some())
    }
}

fn update_tick(widget: &gtk::DrawingArea, state: &Rc<IndicatorState>) {
    let reduced_motion = !adw::is_animations_enabled(widget);
    state.reduced_motion.set(reduced_motion);
    if reduced_motion {
        state.settle_for_reduced_motion();
        state.stop_tick(widget);
        state.apply_accessibility(widget);
        widget.queue_draw();
    } else if state.should_animate(widget) {
        state.ensure_tick(widget);
    } else {
        state.stop_tick(widget);
        state.apply_accessibility(widget);
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct IndicatorSnapshot {
    mode: LoadingIndicatorMode,
    presentation: LoadingIndicatorPresentation,
    size: LoadingIndicatorSize,
    reduced_motion: bool,
    phase: f64,
    progress: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Point {
    x: f64,
    y: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct CubicSegment {
    start: Point,
    control_1: Point,
    control_2: Point,
    end: Point,
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

    fn active_scale(self, presentation: LoadingIndicatorPresentation) -> f64 {
        match (self, presentation) {
            (_, LoadingIndicatorPresentation::Contained) => 0.78,
            (Self::Compact, LoadingIndicatorPresentation::Uncontained) => 0.82,
            (_, LoadingIndicatorPresentation::Uncontained) => 0.86,
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

fn indeterminate_shape_points(phase: f64) -> Vec<Point> {
    let normalized = phase.rem_euclid(1.0);
    let scaled = normalized * INDETERMINATE_SHAPE_COUNT as f64;
    let left = scaled.floor() as usize % INDETERMINATE_SHAPE_COUNT;
    let right = (left + 1) % INDETERMINATE_SHAPE_COUNT;
    let local = eased(scaled - scaled.floor());
    morph_points(
        &indeterminate_shape(left),
        &indeterminate_shape(right),
        local,
    )
}

fn determinate_shape_points(progress: f64) -> Vec<Point> {
    morph_points(&circle_shape(), &soft_burst_shape(), eased(progress))
}

fn morph_points(left: &[Point], right: &[Point], amount: f64) -> Vec<Point> {
    debug_assert_eq!(left.len(), right.len());
    let amount = clamp_progress(amount);
    left.iter()
        .zip(right.iter())
        .map(|(left, right)| Point {
            x: left.x + (right.x - left.x) * amount,
            y: left.y + (right.y - left.y) * amount,
        })
        .collect()
}

fn indeterminate_shape(index: usize) -> Vec<Point> {
    match index % INDETERMINATE_SHAPE_COUNT {
        0 => circle_shape(),
        1 => radial_shape(0.84, 1.12, 4.0, PI / 4.0),
        2 => radial_shape(0.80, 1.18, 4.0, 0.0),
        3 => radial_shape(0.78, 1.18, 3.0, -PI / 2.0),
        4 => radial_shape(0.81, 1.13, 5.0, -PI / 2.0),
        5 => oval_shape(),
        _ => soft_burst_shape(),
    }
}

fn circle_shape() -> Vec<Point> {
    radial_shape(1.0, 1.0, 0.0, 0.0)
}

fn soft_burst_shape() -> Vec<Point> {
    radial_shape(0.74, 1.16, 8.0, 0.0)
}

fn radial_shape(base: f64, accent: f64, lobes: f64, offset: f64) -> Vec<Point> {
    let mut points = Vec::with_capacity(POINT_COUNT);
    for index in 0..POINT_COUNT {
        let angle = index as f64 / POINT_COUNT as f64 * TAU;
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
    let mut points = Vec::with_capacity(POINT_COUNT);
    for index in 0..POINT_COUNT {
        let angle = index as f64 / POINT_COUNT as f64 * TAU;
        points.push(Point {
            x: 0.5 + angle.cos() * 0.38,
            y: 0.5 + angle.sin() * 0.25,
        });
    }
    points
}

fn rounded_segments(points: &[Point]) -> Vec<CubicSegment> {
    if points.len() < 3 {
        return Vec::new();
    }

    let count = points.len();
    (0..count)
        .map(|index| {
            let previous = points[(index + count - 1) % count];
            let start = points[index];
            let end = points[(index + 1) % count];
            let following = points[(index + 2) % count];
            let control_1 = Point {
                x: start.x + (end.x - previous.x) * CURVE_TENSION / 6.0,
                y: start.y + (end.y - previous.y) * CURVE_TENSION / 6.0,
            };
            let control_2 = Point {
                x: end.x - (following.x - start.x) * CURVE_TENSION / 6.0,
                y: end.y - (following.y - start.y) * CURVE_TENSION / 6.0,
            };
            CubicSegment {
                start,
                control_1,
                control_2,
                end,
            }
        })
        .collect()
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
    let scale = snapshot.size.active_scale(snapshot.presentation);
    let (points, rotation) = match snapshot.mode {
        LoadingIndicatorMode::Indeterminate => {
            let phase = if snapshot.reduced_motion {
                0.0
            } else {
                snapshot.phase
            };
            let points = if snapshot.reduced_motion {
                circle_shape()
            } else {
                indeterminate_shape_points(phase)
            };
            (
                points,
                if snapshot.reduced_motion {
                    0.0
                } else {
                    phase * TAU
                },
            )
        }
        LoadingIndicatorMode::Determinate(_) => {
            let progress = clamp_progress(snapshot.progress);
            (
                determinate_shape_points(progress),
                if snapshot.reduced_motion {
                    0.0
                } else {
                    -PI * progress
                },
            )
        }
    };

    let color = widget.color();
    let _ = context.save();
    context.translate(width as f64 / 2.0, height as f64 / 2.0);
    context.rotate(rotation);
    context.scale(size * scale, size * scale);
    context.translate(-0.5, -0.5);
    context.set_source_rgba(
        color.red() as f64,
        color.green() as f64,
        color.blue() as f64,
        0.98,
    );
    draw_rounded_points(context, &points);
    let _ = context.fill();
    let _ = context.restore();
}

fn draw_rounded_points(context: &cairo::Context, points: &[Point]) {
    let segments = rounded_segments(points);
    if let Some(first) = segments.first() {
        context.new_path();
        context.move_to(first.start.x, first.start.y);
        for segment in &segments {
            context.curve_to(
                segment.control_1.x,
                segment.control_1.y,
                segment.control_2.x,
                segment.control_2.y,
                segment.end.x,
                segment.end.y,
            );
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

    fn state(mode: LoadingIndicatorMode) -> IndicatorState {
        IndicatorState::new(
            mode,
            LoadingIndicatorPresentation::Uncontained,
            LoadingIndicatorSize::Standard,
        )
    }

    #[test]
    fn progress_values_are_clamped() {
        assert_eq!(clamp_progress(-1.0), 0.0);
        assert_eq!(clamp_progress(1.4), 1.0);
        assert_eq!(clamp_progress(f64::NAN), 0.0);
    }

    #[test]
    fn determinate_endpoints_use_circle_and_soft_burst() {
        assert_eq!(determinate_shape_points(0.0), circle_shape());
        assert_eq!(determinate_shape_points(1.0), soft_burst_shape());
    }

    #[test]
    fn adjacent_indeterminate_shape_interpolation_is_continuous() {
        for index in 0..INDETERMINATE_SHAPE_COUNT {
            let left = indeterminate_shape(index);
            let right = indeterminate_shape((index + 1) % INDETERMINATE_SHAPE_COUNT);
            assert_eq!(morph_points(&left, &right, 0.0), left);
            assert_eq!(morph_points(&left, &right, 1.0), right);
        }
    }

    #[test]
    fn all_shapes_stay_inside_normalized_viewport() {
        for index in 0..INDETERMINATE_SHAPE_COUNT {
            let points = indeterminate_shape(index);
            let (min_x, max_x, min_y, max_y) = normalized_bounds(&points);
            assert!(min_x >= 0.0);
            assert!(max_x <= 1.0);
            assert!(min_y >= 0.0);
            assert!(max_y <= 1.0);
        }
        for progress in [0.0, 0.25, 0.5, 0.75, 1.0] {
            let points = determinate_shape_points(progress);
            let (min_x, max_x, min_y, max_y) = normalized_bounds(&points);
            assert!(min_x >= 0.0);
            assert!(max_x <= 1.0);
            assert!(min_y >= 0.0);
            assert!(max_y <= 1.0);
        }
    }

    #[test]
    fn shape_center_stays_stable_during_morphs() {
        for phase in [0.0, 0.1, 0.25, 0.5, 0.75, 0.9] {
            let center = normalized_center(&indeterminate_shape_points(phase));
            assert!((center.x - 0.5).abs() < 0.0001);
            assert!((center.y - 0.5).abs() < 0.0001);
        }
        for progress in [0.0, 0.25, 0.5, 0.75, 1.0] {
            let center = normalized_center(&determinate_shape_points(progress));
            assert!((center.x - 0.5).abs() < 0.0001);
            assert!((center.y - 0.5).abs() < 0.0001);
        }
    }

    #[test]
    fn rounded_segments_form_one_closed_continuous_path() {
        let points = soft_burst_shape();
        let segments = rounded_segments(&points);
        assert_eq!(segments.len(), points.len());
        for index in 0..segments.len() {
            let current = segments[index];
            let next = segments[(index + 1) % segments.len()];
            assert_eq!(current.end, next.start);
            for value in [
                current.control_1.x,
                current.control_1.y,
                current.control_2.x,
                current.control_2.y,
            ] {
                assert!(value.is_finite());
            }
        }
    }

    #[test]
    fn indeterminate_timing_is_refresh_rate_independent() {
        let sixty_hz = state(LoadingIndicatorMode::Indeterminate);
        for _ in 0..60 {
            sixty_hz.advance_by(1.0 / 60.0);
        }

        let one_twenty_hz = state(LoadingIndicatorMode::Indeterminate);
        for _ in 0..120 {
            one_twenty_hz.advance_by(1.0 / 120.0);
        }

        assert!((sixty_hz.phase.get() - one_twenty_hz.phase.get()).abs() < 0.0001);
    }

    #[test]
    fn determinate_progress_advances_and_converges() {
        let state = state(LoadingIndicatorMode::Determinate(0.0));
        state.set_mode(LoadingIndicatorMode::Determinate(0.75));
        assert!(state.needs_animation());
        for _ in 0..180 {
            state.advance_by(1.0 / 60.0);
        }
        assert!((state.displayed_progress.get() - 0.75).abs() <= PROGRESS_EPSILON);
        assert!(!state.needs_animation());
    }

    #[test]
    fn reduced_motion_settles_determinate_progress_without_animation() {
        let state = state(LoadingIndicatorMode::Determinate(0.0));
        state.set_mode(LoadingIndicatorMode::Determinate(1.0));
        state.set_reduced_motion(true);
        assert_eq!(state.displayed_progress.get(), 1.0);
        assert!(!state.needs_animation());
    }

    #[test]
    fn hidden_unmapped_state_starts_without_tick() {
        let state = state(LoadingIndicatorMode::Indeterminate);
        assert_eq!(state.tick_count(), 0);
    }

    #[test]
    fn style_mapping_covers_all_variants() {
        assert_eq!(
            LoadingIndicatorPresentation::Contained.css_class(),
            "contained"
        );
        assert_eq!(
            LoadingIndicatorPresentation::Uncontained.css_class(),
            "uncontained"
        );
        assert_eq!(LoadingIndicatorSize::Compact.css_class(), "compact");
        assert_eq!(LoadingIndicatorSize::Standard.css_class(), "standard");
        assert_eq!(LoadingIndicatorSize::Large.css_class(), "large");
        assert!(
            LoadingIndicatorSize::Standard.active_scale(LoadingIndicatorPresentation::Contained)
                > 0.7
        );
    }

    #[test]
    fn accessibility_progress_is_percent_based() {
        assert_eq!(accessible_progress_value(0.0), 0.0);
        assert_eq!(accessible_progress_value(0.25), 25.0);
        assert_eq!(accessible_progress_value(1.0), 100.0);
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
