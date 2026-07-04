//! UI-state contract for the MetroList-style Home V3 shell.
//!
//! This keeps the decision about loading, empty and feed states separate from
//! the GTK renderer so the old Home path does not become the implicit fallback.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum HomeV3ShellState {
    Loading,
    Empty,
    Feed,
}

pub(crate) fn shell_state(loading: bool, section_count: usize) -> HomeV3ShellState {
    if loading && section_count == 0 {
        HomeV3ShellState::Loading
    } else if section_count == 0 {
        HomeV3ShellState::Empty
    } else {
        HomeV3ShellState::Feed
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct HomeV3ShellSummary {
    pub state: HomeV3ShellState,
    pub chip_count: usize,
    pub section_count: usize,
    pub has_continuation: bool,
}

pub(crate) fn shell_summary(
    loading: bool,
    chip_count: usize,
    section_count: usize,
    has_continuation: bool,
) -> HomeV3ShellSummary {
    HomeV3ShellSummary {
        state: shell_state(loading, section_count),
        chip_count,
        section_count,
        has_continuation,
    }
}
