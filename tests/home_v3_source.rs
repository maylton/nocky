#[path = "../src/youtube/home_v3.rs"]
mod home_v3;
#[path = "../src/youtube/home_v3_adapter.rs"]
mod home_v3_adapter;
#[path = "../src/youtube/home_v3_source.rs"]
mod home_v3_source;

use home_v3_adapter::{HomeV3SourceItem, HomeV3SourcePage, HomeV3SourceSection};
use home_v3_source::{resolve_home_v3_source, HomeV3FeedOrigin};

fn section_with_title(title: &str) -> HomeV3SourceSection {
    HomeV3SourceSection {
        title: title.to_string(),
        items: vec![HomeV3SourceItem {
            title: "Item".to_string(),
            ..HomeV3SourceItem::default()
        }],
        ..HomeV3SourceSection::default()
    }
}

fn empty_section_with_title(title: &str) -> HomeV3SourceSection {
    HomeV3SourceSection {
        title: title.to_string(),
        items: Vec::new(),
        ..HomeV3SourceSection::default()
    }
}

fn page_with_titles(titles: &[&str]) -> HomeV3SourcePage {
    HomeV3SourcePage {
        sections: titles
            .iter()
            .map(|title| section_with_title(title))
            .collect(),
        ..HomeV3SourcePage::default()
    }
}

fn page_with_title(title: &str) -> HomeV3SourcePage {
    page_with_titles(&[title])
}

#[test]
fn native_source_wins_when_at_least_as_complete() {
    let resolved = resolve_home_v3_source(
        Some(page_with_titles(&["Native A", "Native B"])),
        page_with_title("Legacy"),
    );

    assert_eq!(resolved.origin, HomeV3FeedOrigin::Native);
    assert_eq!(resolved.page.sections[0].title, "Native A");
}

#[test]
fn legacy_bridge_wins_when_it_has_more_visible_sections() {
    let resolved = resolve_home_v3_source(
        Some(page_with_title("Native")),
        page_with_titles(&["Legacy A", "Legacy B"]),
    );

    assert_eq!(resolved.origin, HomeV3FeedOrigin::LegacyBridge);
    assert_eq!(resolved.page.sections[0].title, "Legacy A");
}

#[test]
fn empty_native_sections_do_not_count_as_visible_sections() {
    let native = HomeV3SourcePage {
        sections: vec![empty_section_with_title("Native empty")],
        ..HomeV3SourcePage::default()
    };

    let resolved = resolve_home_v3_source(Some(native), page_with_title("Legacy"));

    assert_eq!(resolved.origin, HomeV3FeedOrigin::LegacyBridge);
    assert_eq!(resolved.page.sections[0].title, "Legacy");
}

#[test]
fn legacy_bridge_is_used_when_native_source_is_absent() {
    let resolved = resolve_home_v3_source(None, page_with_title("Legacy"));

    assert_eq!(resolved.origin, HomeV3FeedOrigin::LegacyBridge);
    assert_eq!(resolved.page.sections[0].title, "Legacy");
}
