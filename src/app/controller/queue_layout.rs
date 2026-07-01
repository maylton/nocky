//! Small, testable geometry helpers for Queue 2.0 presentation.

pub(super) fn queue_drag_target(
    origin: usize,
    offset_y: f64,
    row_height: i32,
    item_count: usize,
) -> usize {
    if item_count == 0 {
        return 0;
    }

    let row_height = f64::from(row_height.max(1));
    let delta = (offset_y / row_height).round() as isize;
    (origin as isize + delta).clamp(0, item_count.saturating_sub(1) as isize) as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drag_target_is_constant_time_and_clamped() {
        assert_eq!(queue_drag_target(3, 96.0, 48, 10), 5);
        assert_eq!(queue_drag_target(3, -500.0, 48, 10), 0);
        assert_eq!(queue_drag_target(8, 500.0, 48, 10), 9);
        assert_eq!(queue_drag_target(0, 10.0, 0, 0), 0);
    }
}
