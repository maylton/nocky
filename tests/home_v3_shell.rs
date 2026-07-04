#[path = "../src/youtube/home_v3_shell.rs"]
mod home_v3_shell;

use home_v3_shell::{shell_state, shell_summary, HomeV3ShellState};

#[test]
fn shell_loading_requires_no_feed_sections_yet() {
    assert_eq!(shell_state(true, 0), HomeV3ShellState::Loading);
    assert_eq!(shell_state(true, 2), HomeV3ShellState::Feed);
}

#[test]
fn shell_empty_does_not_fall_back_to_old_home() {
    let summary = shell_summary(false, 3, 0, false);

    assert_eq!(summary.state, HomeV3ShellState::Empty);
    assert_eq!(summary.chip_count, 3);
    assert_eq!(summary.section_count, 0);
    assert!(!summary.has_continuation);
}

#[test]
fn shell_feed_preserves_metrolist_continuation_signal() {
    let summary = shell_summary(false, 2, 4, true);

    assert_eq!(summary.state, HomeV3ShellState::Feed);
    assert_eq!(summary.chip_count, 2);
    assert_eq!(summary.section_count, 4);
    assert!(summary.has_continuation);
}
