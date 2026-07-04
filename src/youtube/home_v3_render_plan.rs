//! Render-plan contract for the MetroList-style Home V3 UI.
//!
//! The GTK renderer should consume this plan instead of making fallback
//! decisions inline. That keeps the old Home V2 path out of the new surface.

use super::home_v3::HomeV3Page;
use super::home_v3_shell::{shell_summary, HomeV3ShellState};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum HomeV3RenderBlock {
    Chips {
        count: usize,
    },
    Loading,
    Empty,
    Section {
        index: usize,
        title: String,
        item_count: usize,
    },
    Continuation,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct HomeV3RenderPlan {
    pub blocks: Vec<HomeV3RenderBlock>,
}

pub(crate) fn render_plan(page: &HomeV3Page, loading: bool) -> HomeV3RenderPlan {
    let summary = shell_summary(
        loading,
        page.chips.len(),
        page.sections.len(),
        page.has_continuation(),
    );
    let mut blocks = Vec::new();

    if summary.chip_count > 0 {
        blocks.push(HomeV3RenderBlock::Chips {
            count: summary.chip_count,
        });
    }

    match summary.state {
        HomeV3ShellState::Loading => blocks.push(HomeV3RenderBlock::Loading),
        HomeV3ShellState::Empty => blocks.push(HomeV3RenderBlock::Empty),
        HomeV3ShellState::Feed => {
            for (index, section) in page.sections.iter().enumerate() {
                blocks.push(HomeV3RenderBlock::Section {
                    index,
                    title: section.title.clone(),
                    item_count: section.items.len(),
                });
            }
            if summary.has_continuation {
                blocks.push(HomeV3RenderBlock::Continuation);
            }
        }
    }

    HomeV3RenderPlan { blocks }
}
