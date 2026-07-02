use super::keyline::KeylineKind;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum MaterialCarouselStrategy {
    Hero,
    #[default]
    MultiBrowse,
    Uncontained,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct StrategyKeyline {
    pub position: f64,
    pub item_size: f64,
    pub kind: KeylineKind,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct FeaturedCardMetrics {
    pub viewport_width: f64,
    pub large_width: f64,
    pub medium_width: f64,
    pub small_width: f64,
    pub card_height: f64,
    pub artwork_width: f64,
    pub artwork_height: f64,
    pub horizontal_padding: f64,
    pub vertical_padding: f64,
}

impl FeaturedCardMetrics {
    pub(crate) fn for_viewport(viewport_width: f64) -> Self {
        let viewport_width = finite_non_negative(viewport_width);
        let large_width = (viewport_width * 0.42).clamp(300.0, 420.0);
        let medium_width = large_width * 0.68;
        let small_width = (large_width * 0.32).clamp(72.0, 112.0);
        let card_height = (large_width * 0.72).clamp(230.0, 310.0);
        let artwork_width = large_width;
        let artwork_height = (card_height * 0.68).clamp(160.0, 220.0);

        Self {
            viewport_width,
            large_width,
            medium_width,
            small_width,
            card_height,
            artwork_width,
            artwork_height,
            horizontal_padding: 16.0,
            vertical_padding: 14.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct StrategyParameters {
    pub small_ratio: f64,
    pub medium_ratio: f64,
    pub large_ratio: f64,
    pub focal_viewport_ratio: f64,
    pub medium_band_base_ratio: f64,
    pub edge_peek_base_ratio: f64,
    pub multi_large_breakpoint_base_ratio: f64,
}

impl MaterialCarouselStrategy {
    /// Hero is intentionally asymmetric: one large item dominates around 42%
    /// of the viewport, while the trailing side leaves room for the next item
    /// preview. Ratios are item-width multipliers, not CSS scale values.
    pub(crate) const HERO: StrategyParameters = StrategyParameters {
        small_ratio: 0.44,
        medium_ratio: 0.68,
        large_ratio: 1.0,
        focal_viewport_ratio: 0.42,
        medium_band_base_ratio: 0.86,
        edge_peek_base_ratio: 0.48,
        multi_large_breakpoint_base_ratio: 0.0,
    };

    /// MultiBrowse is symmetric: small previews sit on both viewport edges,
    /// medium zones sit inside them, and the number of large keylines adapts
    /// to the available viewport width.
    pub(crate) const MULTI_BROWSE: StrategyParameters = StrategyParameters {
        small_ratio: 0.46,
        medium_ratio: 0.72,
        large_ratio: 1.0,
        focal_viewport_ratio: 0.5,
        medium_band_base_ratio: 0.74,
        edge_peek_base_ratio: 0.46,
        multi_large_breakpoint_base_ratio: 3.35,
    };

    /// Uncontained keeps every item at its base width and leaves the layout
    /// manager's spacing untouched.
    pub(crate) const UNCONTAINED: StrategyParameters = StrategyParameters {
        small_ratio: 1.0,
        medium_ratio: 1.0,
        large_ratio: 1.0,
        focal_viewport_ratio: 0.5,
        medium_band_base_ratio: 0.0,
        edge_peek_base_ratio: 0.0,
        multi_large_breakpoint_base_ratio: 0.0,
    };

    pub(crate) const fn parameters(self) -> StrategyParameters {
        match self {
            Self::Hero => Self::HERO,
            Self::MultiBrowse => Self::MULTI_BROWSE,
            Self::Uncontained => Self::UNCONTAINED,
        }
    }

    pub(crate) fn keyline_specs(
        self,
        viewport_width: f64,
        base_item_width: f64,
        spacing: f64,
    ) -> Vec<StrategyKeyline> {
        match self {
            Self::Hero => hero_specs(viewport_width, base_item_width),
            Self::MultiBrowse => multi_browse_specs(viewport_width, base_item_width, spacing),
            Self::Uncontained => {
                vec![StrategyKeyline {
                    position: viewport_width / 2.0,
                    item_size: base_item_width,
                    kind: KeylineKind::Large,
                }]
            }
        }
    }

    pub(crate) fn leading_padding(self, viewport_width: f64) -> f64 {
        match self {
            Self::Hero => {
                let viewport_width = finite_non_negative(viewport_width);
                let metrics = FeaturedCardMetrics::for_viewport(viewport_width);
                let focal = viewport_width * Self::HERO.focal_viewport_ratio;
                (focal - metrics.large_width / 2.0).max(0.0)
            }
            Self::MultiBrowse | Self::Uncontained => 0.0,
        }
    }
}

fn hero_specs(viewport_width: f64, _base_item_width: f64) -> Vec<StrategyKeyline> {
    let params = MaterialCarouselStrategy::HERO;
    let metrics = FeaturedCardMetrics::for_viewport(viewport_width);
    let large_width = metrics.large_width;
    let medium_width = metrics.medium_width;
    let small_width = metrics.small_width;
    let focal = viewport_width * params.focal_viewport_ratio;
    let leading_medium = (focal - large_width * 0.42).max(small_width);
    let trailing_medium = (focal + large_width * 0.58).min(viewport_width - small_width);
    let trailing_medium_far = viewport_width + large_width * 0.65;
    let trailing_medium_far_width = large_width * 0.85;

    vec![
        spec(0.0, small_width, KeylineKind::Small),
        spec(leading_medium, medium_width, KeylineKind::Medium),
        spec(focal, large_width, KeylineKind::Large),
        spec(trailing_medium, medium_width, KeylineKind::Medium),
        spec(viewport_width, small_width, KeylineKind::Small),
        spec(
            trailing_medium_far,
            trailing_medium_far_width,
            KeylineKind::Medium,
        ),
    ]
}

fn multi_browse_specs(
    viewport_width: f64,
    base_item_width: f64,
    spacing: f64,
) -> Vec<StrategyKeyline> {
    let params = MaterialCarouselStrategy::MULTI_BROWSE;
    let center = viewport_width * params.focal_viewport_ratio;
    let medium_offset = base_item_width * params.medium_band_base_ratio;
    let large_count =
        if viewport_width >= base_item_width * params.multi_large_breakpoint_base_ratio {
            2
        } else {
            1
        };

    let mut specs = vec![
        spec(
            0.0,
            base_item_width * params.small_ratio,
            KeylineKind::Small,
        ),
        spec(
            medium_offset.min(center),
            base_item_width * params.medium_ratio,
            KeylineKind::Medium,
        ),
    ];

    if large_count == 2 {
        let large_gap = base_item_width + spacing;
        specs.push(spec(
            (center - large_gap / 2.0).max(0.0),
            base_item_width * params.large_ratio,
            KeylineKind::Large,
        ));
        specs.push(spec(
            (center + large_gap / 2.0).min(viewport_width),
            base_item_width * params.large_ratio,
            KeylineKind::Large,
        ));
    } else {
        specs.push(spec(
            center,
            base_item_width * params.large_ratio,
            KeylineKind::Large,
        ));
    }

    specs.push(spec(
        (viewport_width - medium_offset).max(center),
        base_item_width * params.medium_ratio,
        KeylineKind::Medium,
    ));
    specs.push(spec(
        viewport_width,
        base_item_width * params.small_ratio,
        KeylineKind::Small,
    ));
    specs
}

fn spec(position: f64, item_size: f64, kind: KeylineKind) -> StrategyKeyline {
    StrategyKeyline {
        position,
        item_size,
        kind,
    }
}

fn finite_non_negative(value: f64) -> f64 {
    if value.is_finite() && value > 0.0 {
        value
    } else {
        0.0
    }
}
