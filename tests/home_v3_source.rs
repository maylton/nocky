#[path = "../src/youtube/home_v3.rs"]
mod home_v3;
#[path = "../src/youtube/home_v3_adapter.rs"]
mod home_v3_adapter;
#[path = "../src/youtube/home_v3_source.rs"]
mod home_v3_source;

use home_v3_adapter::{HomeV3SourceItem, HomeV3SourcePage, HomeV3SourceSection};
use home_v3_source::{resolve_home_v3_source, HomeV3FeedOrigin};

fn page_with_title(title: &str) -> HomeV3SourcePage {
    HomeV3SourcePage {
        sections: vec![HomeV3SourceSection {
            title: title.to_string(),
            items: vec![HomeV3SourceItem {
                title: "Item".to_string(),
                ..HomeV3SourceItem::default()
            }],
            ..HomeV3SourceSection::default()
        }],
        ..HomeV3SourcePage::default()
    }
}

#[test]
fn native_source_wins_when_available() {
    let resolved =
        resolve_home_v3_source(Some(page_with_title("Native")), page_with_title("Legacy"));

    assert_eq!(resolved.origin, HomeV3FeedOrigin::Native);
    assert_eq!(resolved.page.sections[0].title, "Native");
}

#[test]
fn native_empty_source_still_blocks_legacy_fallback() {
    let resolved =
        resolve_home_v3_source(Some(HomeV3SourcePage::default()), page_with_title("Legacy"));

    assert_eq!(resolved.origin, HomeV3FeedOrigin::Native);
    assert!(resolved.page.sections.is_empty());
}

#[test]
fn legacy_bridge_is_used_only_when_native_source_is_absent() {
    let resolved = resolve_home_v3_source(None, page_with_title("Legacy"));

    assert_eq!(resolved.origin, HomeV3FeedOrigin::LegacyBridge);
    assert_eq!(resolved.page.sections[0].title, "Legacy");
}
