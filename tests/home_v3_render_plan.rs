#[path = "../src/youtube/home_v3.rs"]
mod home_v3;
#[path = "../src/youtube/home_v3_render_plan.rs"]
mod home_v3_render_plan;
#[path = "../src/youtube/home_v3_shell.rs"]
mod home_v3_shell;

use home_v3::{HomeV3Chip, HomeV3Item, HomeV3Page, HomeV3Section, HomeV3SectionLayout};
use home_v3_render_plan::{render_plan, HomeV3RenderBlock};

#[test]
fn render_plan_shows_loading_without_old_home_sections() {
    let page = HomeV3Page::default();
    let plan = render_plan(&page, true);

    assert_eq!(plan.blocks, vec![HomeV3RenderBlock::Loading]);
}

#[test]
fn render_plan_shows_empty_without_old_home_fallback() {
    let page = HomeV3Page {
        chips: vec![HomeV3Chip {
            title: "All".to_string(),
            params: String::new(),
        }],
        ..HomeV3Page::default()
    };
    let plan = render_plan(&page, false);

    assert_eq!(
        plan.blocks,
        vec![
            HomeV3RenderBlock::Chips { count: 1 },
            HomeV3RenderBlock::Empty
        ]
    );
}

#[test]
fn render_plan_keeps_metrolist_feed_order_and_continuation() {
    let page = HomeV3Page {
        sections: vec![
            HomeV3Section {
                title: "Quick picks".to_string(),
                layout: HomeV3SectionLayout::Carousel,
                items: vec![HomeV3Item {
                    title: "Song".to_string(),
                    ..HomeV3Item::default()
                }],
            },
            HomeV3Section {
                title: "Listen again".to_string(),
                layout: HomeV3SectionLayout::List,
                items: vec![HomeV3Item {
                    title: "Track".to_string(),
                    ..HomeV3Item::default()
                }],
            },
        ],
        continuation: "next".to_string(),
        ..HomeV3Page::default()
    };
    let plan = render_plan(&page, false);

    assert_eq!(
        plan.blocks,
        vec![
            HomeV3RenderBlock::Section {
                index: 0,
                title: "Quick picks".to_string(),
                item_count: 1,
            },
            HomeV3RenderBlock::Section {
                index: 1,
                title: "Listen again".to_string(),
                item_count: 1,
            },
            HomeV3RenderBlock::Continuation,
        ]
    );
}
