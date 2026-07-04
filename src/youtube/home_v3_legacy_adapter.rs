#![allow(dead_code)]

//! Temporary bridge from the legacy YouTube Home payload to the Home V3 source
//! contract. This keeps the renderer on `HomeV3Page` while the native helper /
//! parser is introduced.

use super::{
    feed::YouTubeHomePage,
    home_v3_adapter::{HomeV3SourceChip, HomeV3SourceItem, HomeV3SourcePage, HomeV3SourceSection},
};

pub(crate) fn legacy_youtube_home_page_source(page: &YouTubeHomePage) -> HomeV3SourcePage {
    HomeV3SourcePage {
        chips: page
            .chips
            .iter()
            .map(|chip| HomeV3SourceChip {
                title: chip.title.clone(),
                params: chip.params.clone(),
            })
            .collect(),
        sections: page
            .sections
            .iter()
            .map(|section| HomeV3SourceSection {
                title: legacy_section_title(section.title.trim(), section.label.trim()),
                layout: section.layout.clone(),
                items: section
                    .items
                    .iter()
                    .map(|item| HomeV3SourceItem {
                        result_type: item.result_type.clone(),
                        title: item.title.clone(),
                        subtitle: legacy_item_subtitle(
                            item.subtitle.trim(),
                            item.artist.trim(),
                            item.album.trim(),
                        ),
                        video_id: item.video_id.clone(),
                        browse_id: item.browse_id.clone(),
                        album: item.album.clone(),
                        artist: item.artist.clone(),
                        playlist_kind: item.playlist_kind.clone(),
                        params: item.params.clone(),
                        duration_seconds: item.duration_seconds,
                        thumbnail_url: item.thumbnail_url.clone(),
                        cover_path: item.cover_path.clone(),
                    })
                    .collect(),
            })
            .collect(),
        continuation: page.continuation.clone(),
        selected_chip_params: page.selected_chip_params.clone(),
    }
}

fn legacy_section_title(title: &str, label: &str) -> String {
    if !title.is_empty() {
        title.to_string()
    } else {
        label.to_string()
    }
}

fn legacy_item_subtitle(subtitle: &str, artist: &str, album: &str) -> String {
    if !subtitle.is_empty() {
        subtitle.to_string()
    } else if !artist.is_empty() {
        artist.to_string()
    } else {
        album.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::super::{
        feed::{YouTubeHomeChip, YouTubeHomeEndpoint, YouTubeHomeSection},
        YouTubeItem,
    };
    use super::*;

    #[test]
    fn legacy_bridge_preserves_home_v3_source_contract() {
        let source = legacy_youtube_home_page_source(&YouTubeHomePage {
            selected_chip_params: "selected".to_string(),
            chips: vec![YouTubeHomeChip {
                title: "Workout".to_string(),
                params: "selected".to_string(),
                ..YouTubeHomeChip::default()
            }],
            sections: vec![YouTubeHomeSection {
                label: "Fallback title".to_string(),
                layout: "list".to_string(),
                endpoint: YouTubeHomeEndpoint {
                    browse_id: "browse".to_string(),
                    params: "params".to_string(),
                },
                items: vec![YouTubeItem {
                    result_type: "playlist".to_string(),
                    title: "Mix".to_string(),
                    artist: "Artist".to_string(),
                    album: "Album".to_string(),
                    playlist_kind: "library".to_string(),
                    browse_id: "VL123".to_string(),
                    params: "item-params".to_string(),
                    duration_seconds: 123,
                    thumbnail_url: "https://example.invalid/cover.jpg".to_string(),
                    cover_path: "/tmp/cover".to_string(),
                    ..YouTubeItem::default()
                }],
                ..YouTubeHomeSection::default()
            }],
            continuation: "next".to_string(),
            ..YouTubeHomePage::default()
        });

        assert_eq!(source.chips[0].title, "Workout");
        assert_eq!(source.chips[0].params, "selected");
        assert_eq!(source.sections[0].title, "Fallback title");
        assert_eq!(source.sections[0].layout, "list");

        let item = &source.sections[0].items[0];
        assert_eq!(item.result_type, "playlist");
        assert_eq!(item.title, "Mix");
        assert_eq!(item.subtitle, "Artist");
        assert_eq!(item.album, "Album");
        assert_eq!(item.artist, "Artist");
        assert_eq!(item.playlist_kind, "library");
        assert_eq!(item.browse_id, "VL123");
        assert_eq!(item.params, "item-params");
        assert_eq!(item.duration_seconds, 123);
        assert_eq!(item.thumbnail_url, "https://example.invalid/cover.jpg");
        assert_eq!(item.cover_path, "/tmp/cover");
        assert_eq!(source.continuation, "next");
        assert_eq!(source.selected_chip_params, "selected");
    }
}
