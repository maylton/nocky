#![allow(dead_code)]

//! Home V3 source selection boundary.
//!
//! The renderer already consumes `HomeV3Page`. This module decides which
//! `HomeV3SourcePage` should feed that contract. For now, runtime still passes
//! `None` for the native source, so the legacy bridge remains active. Once the
//! native helper/parser is wired, native data must win even when it is empty,
//! preventing accidental fallback to the old Home.

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

pub(crate) fn resolve_home_v3_source(
    native: Option<HomeV3SourcePage>,
    legacy: HomeV3SourcePage,
) -> HomeV3ResolvedSource {
    if let Some(page) = native {
        return HomeV3ResolvedSource {
            origin: HomeV3FeedOrigin::Native,
            page,
        };
    }

    HomeV3ResolvedSource {
        origin: HomeV3FeedOrigin::LegacyBridge,
        page: legacy,
    }
}
