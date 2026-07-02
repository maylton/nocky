use super::strategy::MaterialCarouselStrategy;

const SMALL_RATIO: f64 = 0.68;
const MEDIUM_RATIO: f64 = 0.86;
const LARGE_RATIO: f64 = 1.0;
const MIN_ITEM_WIDTH: f64 = 1.0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum KeylineKind {
    Small,
    Medium,
    Large,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Keyline {
    pub position: f64,
    pub item_size: f64,
    pub kind: KeylineKind,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct KeylineState {
    pub viewport_width: f64,
    pub keylines: Vec<Keyline>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ItemGeometry {
    pub content_x: f64,
    pub viewport_x: f64,
    pub visible_width: f64,
    pub content_offset: f64,
    pub corner_radius: f64,
    pub state: KeylineKind,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct CarouselGeometryInput {
    pub item_count: usize,
    pub viewport_width: f64,
    pub scroll_offset: f64,
    pub base_item_width: f64,
    pub spacing: f64,
    pub leading_padding: f64,
    pub variant: MaterialCarouselStrategy,
}

pub(crate) fn layout_items(input: CarouselGeometryInput) -> Vec<ItemGeometry> {
    if input.item_count == 0 {
        return Vec::new();
    }

    let viewport_width = finite_non_negative(input.viewport_width);
    let scroll_offset = finite_or_zero(input.scroll_offset);
    let base_item_width = finite_positive(input.base_item_width);
    let spacing = finite_non_negative(input.spacing);
    let leading_padding = finite_non_negative(input.leading_padding);

    if input.variant == MaterialCarouselStrategy::Uncontained {
        return (0..input.item_count)
            .map(|index| {
                let logical_content_x =
                    logical_content_x(index, base_item_width, spacing, leading_padding);
                let viewport_x = logical_content_x - scroll_offset;
                ItemGeometry {
                    content_x: logical_content_x,
                    viewport_x,
                    visible_width: base_item_width,
                    content_offset: 0.0,
                    corner_radius: corner_radius(KeylineKind::Large),
                    state: KeylineKind::Large,
                }
            })
            .collect();
    }

    let state = KeylineState::for_strategy(input.variant, viewport_width, base_item_width, spacing);
    let mut accumulated_reduction = 0.0;
    let mut items = Vec::with_capacity(input.item_count);

    for index in 0..input.item_count {
        let logical_content_x = logical_content_x(index, base_item_width, spacing, leading_padding);
        let logical_viewport_x = logical_content_x - scroll_offset;
        let center_x = logical_viewport_x + base_item_width / 2.0;
        let visible_width = state.item_width_at(center_x);
        let visual_x = logical_viewport_x - accumulated_reduction;
        let content_x = visual_x + scroll_offset;
        let content_offset = logical_content_x - content_x;
        let item_state = kind_for_width(visible_width, base_item_width);

        items.push(ItemGeometry {
            content_x,
            viewport_x: visual_x,
            visible_width,
            content_offset,
            corner_radius: corner_radius(item_state),
            state: item_state,
        });

        accumulated_reduction += (base_item_width - visible_width).max(0.0);
    }

    items
}

impl KeylineState {
    pub(crate) fn for_strategy(
        strategy: MaterialCarouselStrategy,
        viewport_width: f64,
        base_item_width: f64,
        spacing: f64,
    ) -> Self {
        let viewport_width = finite_non_negative(viewport_width);
        let base_item_width = finite_positive(base_item_width);
        let spacing = finite_non_negative(spacing);
        let keylines = match strategy {
            MaterialCarouselStrategy::Hero => hero_keylines(viewport_width, base_item_width),
            MaterialCarouselStrategy::MultiBrowse => {
                multi_browse_keylines(viewport_width, base_item_width, spacing)
            }
            MaterialCarouselStrategy::Uncontained => vec![Keyline {
                position: viewport_width / 2.0,
                item_size: base_item_width,
                kind: KeylineKind::Large,
            }],
        };

        Self {
            viewport_width,
            keylines,
        }
    }

    pub(crate) fn item_width_at(&self, position: f64) -> f64 {
        let Some(first) = self.keylines.first() else {
            return MIN_ITEM_WIDTH;
        };
        let position = finite_or_zero(position);

        if position <= first.position {
            return first.item_size;
        }

        for pair in self.keylines.windows(2) {
            let from = pair[0];
            let to = pair[1];
            if position <= to.position {
                let distance = (to.position - from.position).max(MIN_ITEM_WIDTH);
                let t = ((position - from.position) / distance).clamp(0.0, 1.0);
                return lerp(from.item_size, to.item_size, t).max(MIN_ITEM_WIDTH);
            }
        }

        self.keylines
            .last()
            .map(|keyline| keyline.item_size)
            .unwrap_or(MIN_ITEM_WIDTH)
    }
}

fn hero_keylines(viewport_width: f64, base_item_width: f64) -> Vec<Keyline> {
    let center = viewport_width / 2.0;
    let shoulder = (base_item_width + viewport_width * 0.08).min(center);
    keylines_from_specs(
        base_item_width,
        &[
            (0.0, KeylineKind::Small),
            ((center - shoulder).max(0.0), KeylineKind::Medium),
            (center, KeylineKind::Large),
            ((center + shoulder).min(viewport_width), KeylineKind::Medium),
            (viewport_width, KeylineKind::Small),
        ],
    )
}

fn multi_browse_keylines(viewport_width: f64, base_item_width: f64, spacing: f64) -> Vec<Keyline> {
    let center = viewport_width / 2.0;
    let large_gap = base_item_width + spacing;
    let large_count = if viewport_width >= base_item_width * 3.4 {
        2
    } else {
        1
    };

    let mut specs = vec![
        (0.0, KeylineKind::Small),
        ((base_item_width * 0.72).min(center), KeylineKind::Medium),
    ];

    if large_count == 2 {
        specs.push(((center - large_gap / 2.0).max(0.0), KeylineKind::Large));
        specs.push((
            (center + large_gap / 2.0).min(viewport_width),
            KeylineKind::Large,
        ));
    } else {
        specs.push((center, KeylineKind::Large));
    }

    specs.push((
        (viewport_width - base_item_width * 0.72).max(center),
        KeylineKind::Medium,
    ));
    specs.push((viewport_width, KeylineKind::Small));

    keylines_from_specs(base_item_width, &specs)
}

fn keylines_from_specs(base_item_width: f64, specs: &[(f64, KeylineKind)]) -> Vec<Keyline> {
    let mut keylines: Vec<Keyline> = Vec::with_capacity(specs.len());

    for &(position, kind) in specs {
        if let Some(previous) = keylines.last_mut() {
            if distance_is_tiny(previous.position, position) {
                if size_for_kind(kind, base_item_width) > previous.item_size {
                    *previous = Keyline {
                        position,
                        item_size: size_for_kind(kind, base_item_width),
                        kind,
                    };
                }
                continue;
            }
        }

        keylines.push(Keyline {
            position,
            item_size: size_for_kind(kind, base_item_width),
            kind,
        });
    }

    keylines
}

fn logical_content_x(
    index: usize,
    base_item_width: f64,
    spacing: f64,
    leading_padding: f64,
) -> f64 {
    leading_padding + index as f64 * (base_item_width + spacing)
}

fn size_for_kind(kind: KeylineKind, base_item_width: f64) -> f64 {
    let ratio = match kind {
        KeylineKind::Small => SMALL_RATIO,
        KeylineKind::Medium => MEDIUM_RATIO,
        KeylineKind::Large => LARGE_RATIO,
    };
    (base_item_width * ratio).max(MIN_ITEM_WIDTH)
}

fn kind_for_width(width: f64, base_item_width: f64) -> KeylineKind {
    let ratio = width / finite_positive(base_item_width);
    if ratio >= (MEDIUM_RATIO + LARGE_RATIO) / 2.0 {
        KeylineKind::Large
    } else if ratio >= (SMALL_RATIO + MEDIUM_RATIO) / 2.0 {
        KeylineKind::Medium
    } else {
        KeylineKind::Small
    }
}

fn corner_radius(kind: KeylineKind) -> f64 {
    match kind {
        KeylineKind::Small => 18.0,
        KeylineKind::Medium => 22.0,
        KeylineKind::Large => 28.0,
    }
}

fn finite_positive(value: f64) -> f64 {
    if value.is_finite() && value > 0.0 {
        value
    } else {
        MIN_ITEM_WIDTH
    }
}

fn finite_non_negative(value: f64) -> f64 {
    if value.is_finite() && value > 0.0 {
        value
    } else {
        0.0
    }
}

fn finite_or_zero(value: f64) -> f64 {
    if value.is_finite() {
        value
    } else {
        0.0
    }
}

fn distance_is_tiny(a: f64, b: f64) -> bool {
    (a - b).abs() <= f64::EPSILON
}

fn lerp(from: f64, to: f64, t: f64) -> f64 {
    from + (to - from) * t
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f64 = 1.0e-7;

    fn input(
        variant: MaterialCarouselStrategy,
        item_count: usize,
        viewport_width: f64,
        scroll_offset: f64,
    ) -> CarouselGeometryInput {
        CarouselGeometryInput {
            item_count,
            viewport_width,
            scroll_offset,
            base_item_width: 120.0,
            spacing: 12.0,
            leading_padding: 0.0,
            variant,
        }
    }

    fn assert_valid_geometry(items: &[ItemGeometry]) {
        for item in items {
            assert!(item.content_x.is_finite());
            assert!(item.viewport_x.is_finite());
            assert!(item.visible_width.is_finite());
            assert!(item.visible_width > 0.0);
            assert!(item.content_offset.is_finite());
            assert!(item.corner_radius.is_finite());
        }
    }

    fn assert_ordered_without_overlap(items: &[ItemGeometry], spacing: f64) {
        for pair in items.windows(2) {
            let first = pair[0];
            let second = pair[1];
            assert!(second.viewport_x > first.viewport_x);
            let gap = second.viewport_x - (first.viewport_x + first.visible_width);
            assert!(gap >= -EPSILON, "gap {gap}");
            assert!(gap <= spacing + EPSILON, "gap {gap}");
        }
    }

    #[test]
    fn all_widths_and_positions_are_finite_and_positive() {
        for variant in [
            MaterialCarouselStrategy::Hero,
            MaterialCarouselStrategy::MultiBrowse,
            MaterialCarouselStrategy::Uncontained,
        ] {
            for scroll_offset in [-240.0, 0.0, 77.3, 320.8, 900.0] {
                let items = layout_items(input(variant, 9, 640.0, scroll_offset));
                assert_valid_geometry(&items);
            }
        }
    }

    #[test]
    fn item_order_never_inverts_and_neighbors_do_not_overlap() {
        for variant in [
            MaterialCarouselStrategy::Hero,
            MaterialCarouselStrategy::MultiBrowse,
            MaterialCarouselStrategy::Uncontained,
        ] {
            for step in 0..40 {
                let scroll_offset = step as f64 * 19.75;
                let items = layout_items(input(variant, 12, 680.0, scroll_offset));
                assert_ordered_without_overlap(&items, 12.0);
            }
        }
    }

    #[test]
    fn neighbor_gaps_never_exceed_requested_spacing() {
        for variant in [
            MaterialCarouselStrategy::Hero,
            MaterialCarouselStrategy::MultiBrowse,
        ] {
            for scroll_offset in (0..80).map(|step| step as f64 * 7.5) {
                let items = layout_items(input(variant, 16, 720.0, scroll_offset));
                assert_ordered_without_overlap(&items, 12.0);
            }
        }
    }

    #[test]
    fn tiny_scroll_delta_does_not_create_large_jump() {
        for variant in [
            MaterialCarouselStrategy::Hero,
            MaterialCarouselStrategy::MultiBrowse,
        ] {
            for scroll_offset in (0..80).map(|step| step as f64 * 5.0) {
                let before = layout_items(input(variant, 10, 640.0, scroll_offset));
                let after = layout_items(input(variant, 10, 640.0, scroll_offset + 0.1));

                for (left, right) in before.iter().zip(after.iter()) {
                    assert!((left.viewport_x - right.viewport_x).abs() < 0.5);
                    assert!((left.content_x - right.content_x).abs() < 0.5);
                    assert!((left.visible_width - right.visible_width).abs() < 0.5);
                }
            }
        }
    }

    #[test]
    fn crossing_the_center_is_continuous() {
        for variant in [
            MaterialCarouselStrategy::Hero,
            MaterialCarouselStrategy::MultiBrowse,
        ] {
            let base = input(variant, 7, 600.0, 0.0);
            let item_center_at_zero = base.base_item_width / 2.0;
            let scroll_to_center = item_center_at_zero - base.viewport_width / 2.0;
            let before = layout_items(CarouselGeometryInput {
                scroll_offset: scroll_to_center - 0.05,
                ..base
            });
            let after = layout_items(CarouselGeometryInput {
                scroll_offset: scroll_to_center + 0.05,
                ..base
            });

            assert!((before[0].visible_width - after[0].visible_width).abs() < 0.1);
            assert!((before[0].viewport_x - after[0].viewport_x).abs() < 0.2);
        }
    }

    #[test]
    fn hero_has_at_least_one_large_region() {
        let state = KeylineState::for_strategy(MaterialCarouselStrategy::Hero, 640.0, 120.0, 12.0);
        assert!(state
            .keylines
            .iter()
            .any(|keyline| keyline.kind == KeylineKind::Large));
    }

    #[test]
    fn multi_browse_generates_all_keyline_kinds_for_sufficient_viewport() {
        let items = layout_items(input(MaterialCarouselStrategy::MultiBrowse, 8, 640.0, 0.0));
        assert!(items.iter().any(|item| item.state == KeylineKind::Small));
        assert!(items.iter().any(|item| item.state == KeylineKind::Medium));
        assert!(items.iter().any(|item| item.state == KeylineKind::Large));
    }

    #[test]
    fn uncontained_keeps_all_widths_equal_and_positions_uniform() {
        let items = layout_items(input(MaterialCarouselStrategy::Uncontained, 8, 640.0, 37.0));
        assert_valid_geometry(&items);

        for item in &items {
            assert_eq!(item.visible_width, 120.0);
            assert_eq!(item.state, KeylineKind::Large);
        }

        for pair in items.windows(2) {
            let delta = pair[1].viewport_x - pair[0].viewport_x;
            assert!((delta - 132.0).abs() < EPSILON);
        }
    }

    #[test]
    fn symmetric_strategies_have_symmetric_edge_keylines() {
        for variant in [
            MaterialCarouselStrategy::Hero,
            MaterialCarouselStrategy::MultiBrowse,
        ] {
            let state = KeylineState::for_strategy(variant, 640.0, 120.0, 12.0);
            for keyline in &state.keylines {
                let mirrored_position = state.viewport_width - keyline.position;
                let mirrored_width = state.item_width_at(mirrored_position);
                assert!((keyline.item_size - mirrored_width).abs() < EPSILON);
            }
        }
    }

    #[test]
    fn zero_and_one_item_counts_are_handled() {
        let empty = layout_items(input(MaterialCarouselStrategy::Hero, 0, 640.0, 0.0));
        assert!(empty.is_empty());

        let one = layout_items(input(MaterialCarouselStrategy::Hero, 1, 640.0, 0.0));
        assert_eq!(one.len(), 1);
        assert_valid_geometry(&one);
    }

    #[test]
    fn very_narrow_viewport_has_no_negative_or_invalid_geometry() {
        for variant in [
            MaterialCarouselStrategy::Hero,
            MaterialCarouselStrategy::MultiBrowse,
            MaterialCarouselStrategy::Uncontained,
        ] {
            let items = layout_items(input(variant, 5, 8.0, 20.0));
            assert_valid_geometry(&items);
            assert_ordered_without_overlap(&items, 12.0);
        }
    }
}
