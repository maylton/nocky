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
    pub size_ratio: f64,
    pub kind: KeylineKind,
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
                let params = Self::UNCONTAINED;
                vec![StrategyKeyline {
                    position: viewport_width / 2.0,
                    size_ratio: params.large_ratio,
                    kind: KeylineKind::Large,
                }]
            }
        }
    }
}

fn hero_specs(viewport_width: f64, base_item_width: f64) -> Vec<StrategyKeyline> {
    let params = MaterialCarouselStrategy::HERO;
    let focal = viewport_width * params.focal_viewport_ratio;
    let leading_medium = (focal - base_item_width * params.medium_band_base_ratio).max(0.0);
    let trailing_medium = (focal
        + base_item_width * (params.medium_band_base_ratio + params.edge_peek_base_ratio))
        .min(viewport_width);

    vec![
        spec(0.0, params.small_ratio, KeylineKind::Small),
        spec(leading_medium, params.medium_ratio, KeylineKind::Medium),
        spec(focal, params.large_ratio, KeylineKind::Large),
        spec(trailing_medium, params.medium_ratio, KeylineKind::Medium),
        spec(viewport_width, params.small_ratio, KeylineKind::Small),
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
        spec(0.0, params.small_ratio, KeylineKind::Small),
        spec(
            medium_offset.min(center),
            params.medium_ratio,
            KeylineKind::Medium,
        ),
    ];

    if large_count == 2 {
        let large_gap = base_item_width + spacing;
        specs.push(spec(
            (center - large_gap / 2.0).max(0.0),
            params.large_ratio,
            KeylineKind::Large,
        ));
        specs.push(spec(
            (center + large_gap / 2.0).min(viewport_width),
            params.large_ratio,
            KeylineKind::Large,
        ));
    } else {
        specs.push(spec(center, params.large_ratio, KeylineKind::Large));
    }

    specs.push(spec(
        (viewport_width - medium_offset).max(center),
        params.medium_ratio,
        KeylineKind::Medium,
    ));
    specs.push(spec(viewport_width, params.small_ratio, KeylineKind::Small));
    specs
}

fn spec(position: f64, size_ratio: f64, kind: KeylineKind) -> StrategyKeyline {
    StrategyKeyline {
        position,
        size_ratio,
        kind,
    }
}
