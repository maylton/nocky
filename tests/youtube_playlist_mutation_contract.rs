#[path = "../src/youtube/playlist_mutation_contract.rs"]
mod playlist_mutation_contract;

use playlist_mutation_contract::{
    PlaylistMutationBlock, PlaylistMutationRequest, PlaylistMutationRisk,
    PlaylistPrivacy, PlaylistTarget, PlaylistTrackIdentity,
};

fn owned_playlist() -> PlaylistTarget {
    PlaylistTarget {
        playlist_id: "PL-owned".to_string(),
        title: "Road Trip".to_string(),
        owned: true,
    }
}

#[test]
fn private_playlist_creation_is_non_destructive() {
    let request = PlaylistMutationRequest::Create {
        title: "New playlist".to_string(),
        description: String::new(),
        privacy: PlaylistPrivacy::Private,
    };

    assert_eq!(request.risk(), PlaylistMutationRisk::NonDestructive);
    assert_eq!(request.validate(), Ok(()));
}

#[test]
fn invalid_playlist_titles_are_blocked_before_network_access() {
    let request = PlaylistMutationRequest::Create {
        title: "Bad <title>".to_string(),
        description: String::new(),
        privacy: PlaylistPrivacy::Private,
    };

    assert_eq!(
        request.validate(),
        Err(vec![PlaylistMutationBlock::InvalidTitle])
    );
}

#[test]
fn adding_tracks_requires_ownership_and_unique_video_ids() {
    let mut target = owned_playlist();
    target.owned = false;
    let request = PlaylistMutationRequest::AddTracks {
        target,
        video_ids: vec!["video-1".to_string(), "video-1".to_string()],
    };

    assert_eq!(request.risk(), PlaylistMutationRisk::Reversible);
    assert_eq!(
        request.validate(),
        Err(vec![
            PlaylistMutationBlock::NotOwned,
            PlaylistMutationBlock::DuplicateVideoId,
        ])
    );
}

#[test]
fn metadata_edit_requires_at_least_one_change() {
    let request = PlaylistMutationRequest::EditMetadata {
        target: owned_playlist(),
        title: None,
        description: None,
        privacy: None,
    };

    assert_eq!(request.risk(), PlaylistMutationRisk::Reversible);
    assert_eq!(
        request.validate(),
        Err(vec![PlaylistMutationBlock::NoChanges])
    );
}

#[test]
fn removing_tracks_requires_set_video_identity() {
    let request = PlaylistMutationRequest::RemoveTracks {
        target: owned_playlist(),
        tracks: vec![PlaylistTrackIdentity {
            video_id: "video-1".to_string(),
            set_video_id: String::new(),
        }],
    };

    assert_eq!(request.risk(), PlaylistMutationRisk::Destructive);
    assert_eq!(
        request.validate(),
        Err(vec![PlaylistMutationBlock::MissingSetVideoId])
    );
}

#[test]
fn duplicate_track_occurrences_are_distinguished_by_set_video_id() {
    let request = PlaylistMutationRequest::RemoveTracks {
        target: owned_playlist(),
        tracks: vec![
            PlaylistTrackIdentity {
                video_id: "video-1".to_string(),
                set_video_id: "set-video-1".to_string(),
            },
            PlaylistTrackIdentity {
                video_id: "video-1".to_string(),
                set_video_id: "set-video-2".to_string(),
            },
        ],
    };

    assert_eq!(request.validate(), Ok(()));
}

#[test]
fn repeated_set_video_identity_is_blocked() {
    let request = PlaylistMutationRequest::RemoveTracks {
        target: owned_playlist(),
        tracks: vec![
            PlaylistTrackIdentity {
                video_id: "video-1".to_string(),
                set_video_id: "set-video-1".to_string(),
            },
            PlaylistTrackIdentity {
                video_id: "video-2".to_string(),
                set_video_id: "set-video-1".to_string(),
            },
        ],
    };

    assert_eq!(
        request.validate(),
        Err(vec![PlaylistMutationBlock::DuplicateSetVideoId])
    );
}

#[test]
fn track_removal_is_allowed_only_with_complete_identity() {
    let request = PlaylistMutationRequest::RemoveTracks {
        target: owned_playlist(),
        tracks: vec![PlaylistTrackIdentity {
            video_id: "video-1".to_string(),
            set_video_id: "set-video-1".to_string(),
        }],
    };

    assert_eq!(request.validate(), Ok(()));
}

#[test]
fn deletion_requires_exact_playlist_title_confirmation() {
    let request = PlaylistMutationRequest::Delete {
        target: owned_playlist(),
        confirmation: "road trip".to_string(),
    };

    assert_eq!(request.risk(), PlaylistMutationRisk::Destructive);
    assert_eq!(
        request.validate(),
        Err(vec![PlaylistMutationBlock::ConfirmationMismatch])
    );
}

#[test]
fn deletion_with_exact_confirmation_is_eligible_but_not_executed() {
    let target = owned_playlist();
    let request = PlaylistMutationRequest::Delete {
        confirmation: target.title.clone(),
        target,
    };

    assert_eq!(request.validate(), Ok(()));
}
