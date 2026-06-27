//! Compact-footer volume spring motion.
//!
//! This module owns only the Material Expressive geometry animation. Widget
//! reveal state and theme gating remain in `AppController`.

use gtk::glib;
use gtk::prelude::*;
use std::{cell::Cell, rc::Rc, time::Duration};

// nocky_rust_ui_phase3a_compact_volume_motion_v2
// nocky_compact_volume_light_spring_v1
pub(crate) struct CompactVolumeSpring {
    pub(crate) group: gtk::Box,
    pub(crate) generation: Rc<Cell<u64>>,
    pub(crate) token: u64,
    pub(crate) from_width: i32,
    pub(crate) target_width: i32,
    pub(crate) expanding: bool,
    pub(crate) delay_ms: u64,
}

pub(crate) fn run_compact_volume_spring(animation: CompactVolumeSpring) {
    glib::timeout_add_local_once(Duration::from_millis(animation.delay_ms), move || {
        if animation.generation.get() != animation.token {
            return;
        }

        let started_at = Rc::new(Cell::new(0_i64));
        let group = animation.group.clone();
        let animated_group = group.clone();
        let generation = animation.generation.clone();

        group.add_css_class("volume-spring-active");
        group.add_tick_callback(move |_, frame_clock| {
            if generation.get() != animation.token {
                animated_group.remove_css_class("volume-spring-active");
                return glib::ControlFlow::Break;
            }

            let now = frame_clock.frame_time();
            let start = started_at.get();

            if start == 0 {
                started_at.set(now);
                return glib::ControlFlow::Continue;
            }

            let progress = ((now - start) as f64 / 360_000.0).clamp(0.0, 1.0);
            let width = compact_volume_spring_width(
                animation.from_width,
                animation.target_width,
                progress,
                animation.expanding,
            );

            animated_group.set_size_request(width, 52);

            if progress >= 1.0 {
                animated_group.set_size_request(animation.target_width, 52);
                animated_group.remove_css_class("volume-spring-active");
                glib::ControlFlow::Break
            } else {
                glib::ControlFlow::Continue
            }
        });
    });
}

fn compact_volume_spring_width(
    from_width: i32,
    target_width: i32,
    progress: f64,
    expanding: bool,
) -> i32 {
    let overshoot = if expanding { 7.0 } else { -5.0 };
    let rebound = if expanding { -2.5 } else { 2.0 };
    let target = target_width as f64;
    let from = from_width as f64;

    let width = if progress < 0.68 {
        compact_volume_lerp(
            from,
            target + overshoot,
            compact_volume_ease_out_cubic(progress / 0.68),
        )
    } else if progress < 0.86 {
        compact_volume_lerp(
            target + overshoot,
            target + rebound,
            compact_volume_ease_in_out_cubic((progress - 0.68) / 0.18),
        )
    } else {
        compact_volume_lerp(
            target + rebound,
            target,
            compact_volume_ease_out_cubic((progress - 0.86) / 0.14),
        )
    };

    width.round().max(96.0) as i32
}

fn compact_volume_ease_out_cubic(value: f64) -> f64 {
    1.0 - (1.0 - value.clamp(0.0, 1.0)).powi(3)
}

fn compact_volume_ease_in_out_cubic(value: f64) -> f64 {
    let value = value.clamp(0.0, 1.0);

    if value < 0.5 {
        4.0 * value.powi(3)
    } else {
        1.0 - (-2.0 * value + 2.0).powi(3) / 2.0
    }
}

fn compact_volume_lerp(start: f64, end: f64, progress: f64) -> f64 {
    start + (end - start) * progress
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expansion_starts_at_the_current_width() {
        assert_eq!(compact_volume_spring_width(100, 234, 0.0, true), 100);
    }

    #[test]
    fn expansion_settles_at_the_target_width() {
        assert_eq!(compact_volume_spring_width(100, 234, 1.0, true), 234);
    }

    #[test]
    fn expansion_overshoots_before_settling() {
        assert!(compact_volume_spring_width(100, 234, 0.68, true) > 234);
    }

    #[test]
    fn collapse_rebounds_below_the_target() {
        assert!(compact_volume_spring_width(234, 100, 0.68, false) < 100);
    }

    #[test]
    fn width_never_falls_below_the_safe_floor() {
        assert_eq!(compact_volume_spring_width(0, 0, 0.0, false), 96);
    }
}
