#[path = "../src/youtube/like_mutation.rs"]
mod like_mutation;

use like_mutation::{
    LikeMutationPhase, LikeMutationRegistry, LikeMutationStartError,
};

#[test]
fn optimistic_value_is_visible_while_pending() {
    let mut registry = LikeMutationRegistry::default();
    let mutation = registry.begin("video-1", false, true).unwrap();

    assert!(mutation.pending());
    assert!(mutation.visible_value());
    assert_eq!(mutation.phase, LikeMutationPhase::Pending);
}

#[test]
fn duplicate_pending_request_is_blocked() {
    let mut registry = LikeMutationRegistry::default();
    registry.begin("video-1", false, true).unwrap();

    assert_eq!(
        registry.begin("video-1", false, true).unwrap_err(),
        LikeMutationStartError::AlreadyPending
    );
}

#[test]
fn separate_tracks_can_mutate_independently() {
    let mut registry = LikeMutationRegistry::default();
    registry.begin("video-1", false, true).unwrap();
    registry.begin("video-2", true, false).unwrap();

    assert!(registry.get("video-1").unwrap().pending());
    assert!(registry.get("video-2").unwrap().pending());
}

#[test]
fn confirmed_request_keeps_target_value() {
    let mut registry = LikeMutationRegistry::default();
    registry.begin("video-1", false, true).unwrap();

    assert!(registry.confirm("video-1"));
    let mutation = registry.get("video-1").unwrap();
    assert_eq!(mutation.phase, LikeMutationPhase::Confirmed);
    assert!(mutation.visible_value());
    assert!(!mutation.pending());
}

#[test]
fn rollback_restores_previous_value() {
    let mut registry = LikeMutationRegistry::default();
    registry.begin("video-1", true, false).unwrap();

    assert!(registry.rollback("video-1", "request failed"));
    let mutation = registry.get("video-1").unwrap();
    assert_eq!(mutation.phase, LikeMutationPhase::RolledBack);
    assert!(mutation.visible_value());
    assert_eq!(mutation.message, "request failed");
}

#[test]
fn invalid_and_unchanged_requests_are_rejected() {
    let mut registry = LikeMutationRegistry::default();

    assert_eq!(
        registry.begin(" ", false, true).unwrap_err(),
        LikeMutationStartError::MissingId
    );
    assert_eq!(
        registry.begin("video-1", true, true).unwrap_err(),
        LikeMutationStartError::Unchanged
    );
}

#[test]
fn only_finished_requests_can_be_cleared() {
    let mut registry = LikeMutationRegistry::default();
    registry.begin("video-1", false, true).unwrap();
    assert!(!registry.clear_finished("video-1"));

    registry.confirm("video-1");
    assert!(registry.clear_finished("video-1"));
    assert!(registry.get("video-1").is_none());
}
