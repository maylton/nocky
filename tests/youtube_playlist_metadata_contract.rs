#[path = "../src/youtube/playlist_metadata.rs"]
mod playlist_metadata;

use playlist_metadata::{
    YouTubePlaylistMetadata, YouTubePlaylistPrivacy, YouTubePlaylistTrackMetadata,
};

#[test]
fn current_payload_preserves_editability_and_occurrence_identity() {
    let metadata: YouTubePlaylistMetadata = serde_json::from_value(serde_json::json!({
        "playlist_id": "PL-owned",
        "title": "Road Trip",
        "owned": true,
        "privacy": "PRIVATE",
        "editable": true,
        "tracks": [
            {
                "video_id": "video-1",
                "set_video_id": "set-video-1",
                "title": "Song"
            },
            {
                "video_id": "video-1",
                "set_video_id": "set-video-2",
                "title": "Song"
            }
        ]
    }))
    .unwrap();

    assert!(metadata.can_edit());
    assert_eq!(metadata.privacy_kind(), YouTubePlaylistPrivacy::Private);
    assert_eq!(metadata.removable_track_count(), 2);
    assert!(metadata.has_unique_removal_identities());
}

#[test]
fn legacy_payload_without_new_fields_remains_read_only() {
    let metadata: YouTubePlaylistMetadata = serde_json::from_value(serde_json::json!({
        "playlist_id": "PL-legacy",
        "title": "Legacy"
    }))
    .unwrap();

    assert!(!metadata.can_edit());
    assert_eq!(metadata.privacy_kind(), YouTubePlaylistPrivacy::Unknown);
    assert!(metadata.tracks.is_empty());
}

#[test]
fn editable_flag_cannot_override_missing_ownership() {
    let metadata = YouTubePlaylistMetadata {
        playlist_id: "PL-shared".to_string(),
        title: "Shared".to_string(),
        owned: false,
        privacy: "UNLISTED".to_string(),
        editable: true,
        tracks: Vec::new(),
    };

    assert!(!metadata.can_edit());
    assert_eq!(metadata.privacy_kind(), YouTubePlaylistPrivacy::Unlisted);
}

#[test]
fn editable_flag_cannot_override_missing_playlist_id() {
    let metadata = YouTubePlaylistMetadata {
        playlist_id: String::new(),
        title: "Missing ID".to_string(),
        owned: true,
        privacy: "PUBLIC".to_string(),
        editable: true,
        tracks: Vec::new(),
    };

    assert!(!metadata.can_edit());
    assert_eq!(metadata.privacy_kind(), YouTubePlaylistPrivacy::Public);
}

#[test]
fn incomplete_track_identity_is_not_removable() {
    let metadata = YouTubePlaylistMetadata {
        tracks: vec![
            YouTubePlaylistTrackMetadata {
                video_id: "video-1".to_string(),
                set_video_id: String::new(),
                title: "Incomplete".to_string(),
            },
            YouTubePlaylistTrackMetadata {
                video_id: String::new(),
                set_video_id: "set-video-2".to_string(),
                title: "Incomplete".to_string(),
            },
        ],
        ..YouTubePlaylistMetadata::default()
    };

    assert_eq!(metadata.removable_track_count(), 0);
}

#[test]
fn repeated_set_video_identity_is_detected() {
    let metadata = YouTubePlaylistMetadata {
        tracks: vec![
            YouTubePlaylistTrackMetadata {
                video_id: "video-1".to_string(),
                set_video_id: "set-video-1".to_string(),
                title: "First".to_string(),
            },
            YouTubePlaylistTrackMetadata {
                video_id: "video-2".to_string(),
                set_video_id: "set-video-1".to_string(),
                title: "Second".to_string(),
            },
        ],
        ..YouTubePlaylistMetadata::default()
    };

    assert_eq!(metadata.removable_track_count(), 2);
    assert!(!metadata.has_unique_removal_identities());
}

#[test]
fn unknown_privacy_is_version_tolerant() {
    let metadata = YouTubePlaylistMetadata {
        privacy: "FUTURE_VALUE".to_string(),
        ..YouTubePlaylistMetadata::default()
    };

    assert_eq!(metadata.privacy_kind(), YouTubePlaylistPrivacy::Unknown);
}
