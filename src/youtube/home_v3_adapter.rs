#![allow(dead_code)]

//! Adapter contract for converting a feed-shaped source into Home V3.
//!
//! The first implementation uses a small neutral source shape so the contract
//! can be tested before it is wired to the existing YouTube feed structs.

use super::home_v3::{HomeV3Chip, HomeV3Item, HomeV3Page, HomeV3Section, HomeV3SectionLayout};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct HomeV3SourcePage {
    pub chips: Vec<HomeV3SourceChip>,
    pub sections: Vec<HomeV3SourceSection>,
    pub continuation: String,
    pub selected_chip_params: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct HomeV3SourceChip {
    pub title: String,
    pub params: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct HomeV3SourceSection {
    pub title: String,
    pub layout: String,
    pub items: Vec<HomeV3SourceItem>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct HomeV3SourceItem {
    pub result_type: String,
    pub title: String,
    pub subtitle: String,
    pub video_id: String,
    pub browse_id: String,
    pub album: String,
    pub artist: String,
    pub playlist_kind: String,
    pub params: String,
    pub duration_seconds: u64,
    pub thumbnail_url: String,
    pub cover_path: String,
}

pub(crate) fn adapt_source_page(source: HomeV3SourcePage) -> HomeV3Page {
    HomeV3Page {
        chips: source
            .chips
            .into_iter()
            .map(|chip| HomeV3Chip {
                title: chip.title,
                params: chip.params,
            })
            .collect(),
        sections: source
            .sections
            .into_iter()
            .filter(|section| !section.items.is_empty())
            .map(|section| HomeV3Section {
                title: section.title,
                layout: adapt_layout(&section.layout),
                items: section
                    .items
                    .into_iter()
                    .map(|item| HomeV3Item {
                        result_type: item.result_type,
                        title: item.title,
                        subtitle: item.subtitle,
                        video_id: item.video_id,
                        browse_id: item.browse_id,
                        album: item.album,
                        artist: item.artist,
                        playlist_kind: item.playlist_kind,
                        params: item.params,
                        duration_seconds: item.duration_seconds,
                        thumbnail_url: item.thumbnail_url,
                        cover_path: item.cover_path,
                    })
                    .collect(),
            })
            .collect(),
        continuation: source.continuation,
        selected_chip_params: source.selected_chip_params,
    }
}

fn adapt_layout(layout: &str) -> HomeV3SectionLayout {
    if layout.eq_ignore_ascii_case("list") {
        HomeV3SectionLayout::List
    } else {
        HomeV3SectionLayout::Carousel
    }
}
