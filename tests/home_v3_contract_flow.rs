#[path = "../src/youtube/home_v3.rs"]
mod home_v3;
#[path = "../src/youtube/home_v3_action.rs"]
mod home_v3_action;
#[path = "../src/youtube/home_v3_adapter.rs"]
mod home_v3_adapter;
#[path = "../src/youtube/home_v3_render_plan.rs"]
mod home_v3_render_plan;
#[path = "../src/youtube/home_v3_shell.rs"]
mod home_v3_shell;

use home_v3_action::{item_action, HomeV3ItemAction};
use home_v3_adapter::{
    adapt_source_page, HomeV3SourceChip, HomeV3SourceItem, HomeV3SourcePage, HomeV3SourceSection,
};
use home_v3_render_plan::{render_plan, HomeV3RenderBlock};

#[test]
fn home_v3_contract_flow_matches_metrolist_behavior() {
    let page = adapt_source_page(HomeV3SourcePage {
        chips: vec![HomeV3SourceChip {
            title: "All".to_string(),
            params: String::new(),
        }],
        sections: vec![HomeV3SourceSection {
            title: "Quick picks".to_string(),
            layout: "carousel".to_string(),
            items: vec![HomeV3SourceItem {
                title: "Song".to_string(),
                subtitle: "Artist".to_string(),
                thumbnail_url: "https://example.invalid/song.jpg".to_string(),
                video_id: "video-id".to_string(),
                browse_id: String::new(),
                params: String::new(),
            }],
        }],
        continuation: "next-page".to_string(),
        selected_chip_params: String::new(),
    });

    let plan = render_plan(&page, false);

    assert_eq!(
        plan.blocks,
        vec![
            HomeV3RenderBlock::Chips { count: 1 },
            HomeV3RenderBlock::Section {
                index: 0,
                title: "Quick picks".to_string(),
                item_count: 1,
            },
            HomeV3RenderBlock::Continuation,
        ]
    );
    assert_eq!(
        item_action(&page.sections[0].items[0]),
        HomeV3ItemAction::Play {
            video_id: "video-id".to_string(),
        }
    );
}

#[test]
fn empty_contract_flow_stays_inside_home_v3() {
    let page = adapt_source_page(HomeV3SourcePage::default());
    let plan = render_plan(&page, false);

    assert_eq!(plan.blocks, vec![HomeV3RenderBlock::Empty]);
}
