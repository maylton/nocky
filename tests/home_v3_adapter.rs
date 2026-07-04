#[path = "../src/youtube/home_v3.rs"]
mod home_v3;
#[path = "../src/youtube/home_v3_adapter.rs"]
mod home_v3_adapter;

use home_v3::HomeV3SectionLayout;
use home_v3_adapter::{
    adapt_source_page, HomeV3SourceChip, HomeV3SourceItem, HomeV3SourcePage, HomeV3SourceSection,
};

#[test]
fn adapter_preserves_metrolist_feed_contract() {
    let page = adapt_source_page(HomeV3SourcePage {
        chips: vec![HomeV3SourceChip {
            title: "Workout".to_string(),
            params: "chip-params".to_string(),
        }],
        sections: vec![HomeV3SourceSection {
            title: "Quick picks".to_string(),
            layout: "carousel".to_string(),
            items: vec![HomeV3SourceItem {
                title: "Song".to_string(),
                subtitle: "Artist".to_string(),
                thumbnail_url: "https://example.invalid/cover.jpg".to_string(),
                video_id: "video-id".to_string(),
                browse_id: String::new(),
                params: String::new(),
            }],
        }],
        continuation: "next-page".to_string(),
        selected_chip_params: "chip-params".to_string(),
    });

    assert_eq!(page.chips[0].title, "Workout");
    assert_eq!(page.chips[0].params, "chip-params");
    assert_eq!(page.sections[0].title, "Quick picks");
    assert_eq!(page.sections[0].layout, HomeV3SectionLayout::Carousel);
    assert_eq!(page.sections[0].items[0].title, "Song");
    assert_eq!(page.sections[0].items[0].video_id, "video-id");
    assert_eq!(page.continuation, "next-page");
    assert_eq!(page.selected_chip_params, "chip-params");
}

#[test]
fn adapter_drops_empty_sections_instead_of_inventing_home_content() {
    let page = adapt_source_page(HomeV3SourcePage {
        sections: vec![HomeV3SourceSection {
            title: "Empty".to_string(),
            layout: "list".to_string(),
            items: Vec::new(),
        }],
        ..HomeV3SourcePage::default()
    });

    assert!(page.sections.is_empty());
}

#[test]
fn adapter_supports_list_sections_for_track_rows() {
    let page = adapt_source_page(HomeV3SourcePage {
        sections: vec![HomeV3SourceSection {
            title: "Listen again".to_string(),
            layout: "list".to_string(),
            items: vec![HomeV3SourceItem {
                title: "Track".to_string(),
                ..HomeV3SourceItem::default()
            }],
        }],
        ..HomeV3SourcePage::default()
    });

    assert_eq!(page.sections[0].layout, HomeV3SectionLayout::List);
}
