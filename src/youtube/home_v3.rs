#![allow(dead_code)]

//! Clean Home V3 foundation inspired by MetroList.
//!
//! This module intentionally starts isolated from the previous Home renderer.
//! The next commits will wire it into the YouTube Home path once the data
//! contract is stable.

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct HomeV3Page {
    pub chips: Vec<HomeV3Chip>,
    pub sections: Vec<HomeV3Section>,
    pub continuation: String,
    pub selected_chip_params: String,
}

impl HomeV3Page {
    pub(crate) fn has_feed(&self) -> bool {
        !self.sections.is_empty()
    }

    pub(crate) fn has_chips(&self) -> bool {
        !self.chips.is_empty()
    }

    pub(crate) fn has_continuation(&self) -> bool {
        !self.continuation.is_empty()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct HomeV3Chip {
    pub title: String,
    pub params: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct HomeV3Section {
    pub title: String,
    pub layout: HomeV3SectionLayout,
    pub items: Vec<HomeV3Item>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) enum HomeV3SectionLayout {
    #[default]
    Carousel,
    List,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct HomeV3Item {
    pub title: String,
    pub subtitle: String,
    pub thumbnail_url: String,
    pub video_id: String,
    pub browse_id: String,
    pub params: String,
}
