#[path = "../src/youtube/home_v3.rs"]
mod home_v3;

use home_v3::{HomeV3Chip, HomeV3Item, HomeV3Page, HomeV3Section, HomeV3SectionLayout};

#[test]
fn home_v3_page_preserves_metrolist_feed_shape() {
    let page = HomeV3Page {
        chips: vec![HomeV3Chip {
            title: "Energize".to_string(),
            params: "EgWKAQIIAWoKEAUQCRADEAQQBQ".to_string(),
        }],
        sections: vec![HomeV3Section {
            title: "Quick picks".to_string(),
            layout: HomeV3SectionLayout::Carousel,
            items: vec![HomeV3Item {
                title: "Song title".to_string(),
                subtitle: "Artist".to_string(),
                thumbnail_url: "https://example.invalid/thumb.jpg".to_string(),
                video_id: "video123".to_string(),
                browse_id: String::new(),
                params: String::new(),
                ..HomeV3Item::default()
            }],
        }],
        continuation: "next-token".to_string(),
        selected_chip_params: String::new(),
    };

    assert_eq!(page.chips[0].title, "Energize");
    assert_eq!(page.sections[0].title, "Quick picks");
    assert_eq!(page.sections[0].layout, HomeV3SectionLayout::Carousel);
    assert_eq!(page.sections[0].items[0].video_id, "video123");
    assert_eq!(page.continuation, "next-token");
    assert!(page.has_chips());
    assert!(page.has_feed());
    assert!(page.has_continuation());
}

#[test]
fn home_v3_page_starts_empty_without_falling_back_to_v2() {
    let page = HomeV3Page::default();

    assert!(page.chips.is_empty());
    assert!(page.sections.is_empty());
    assert!(page.continuation.is_empty());
    assert!(page.selected_chip_params.is_empty());
    assert!(!page.has_chips());
    assert!(!page.has_feed());
    assert!(!page.has_continuation());
}
