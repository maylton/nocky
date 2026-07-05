#![allow(dead_code)]

//! Home V3 source selection boundary.
//!
//! The renderer already consumes `HomeV3Page`. This module decides which
//! `HomeV3SourcePage` should feed that contract.
//!
//! The native helper/parser is still narrower than the legacy structured Home
//! bridge for some YouTube Music shelves. Prefer native data only when it is at
//! least as complete as the legacy bridge, so deeper recommendation shelves do
//! not disappear from the visible Home.

use super::home_v3_adapter::HomeV3SourcePage;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum HomeV3FeedOrigin {
    Native,
    LegacyBridge,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct HomeV3ResolvedSource {
    pub origin: HomeV3FeedOrigin,
    pub page: HomeV3SourcePage,
}

fn visible_section_count(page: &HomeV3SourcePage) -> usize {
    page.sections
        .iter()
        .filter(|section| !section.items.is_empty())
        .count()
}

pub(crate) fn resolve_home_v3_source(
    native: Option<HomeV3SourcePage>,
    legacy: HomeV3SourcePage,
) -> HomeV3ResolvedSource {
    let legacy_section_count = visible_section_count(&legacy);

    if let Some(page) = native {
        if visible_section_count(&page) >= legacy_section_count {
            return HomeV3ResolvedSource {
                origin: HomeV3FeedOrigin::Native,
                page,
            };
        }
    }

    HomeV3ResolvedSource {
        origin: HomeV3FeedOrigin::LegacyBridge,
        page: legacy,
    }
}
