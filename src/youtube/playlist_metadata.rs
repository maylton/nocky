#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum YouTubePlaylistPrivacy {
    Private,
    Unlisted,
    Public,
    #[default]
    Unknown,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct YouTubePlaylistTrackMetadata {
    pub video_id: String,
    pub set_video_id: String,
    pub title: String,
}

impl YouTubePlaylistTrackMetadata {
    pub fn has_removal_identity(&self) -> bool {
        !self.video_id.trim().is_empty() && !self.set_video_id.trim().is_empty()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct YouTubePlaylistMetadata {
    pub playlist_id: String,
    pub title: String,
    pub owned: bool,
    pub privacy: String,
    pub editable: bool,
    pub tracks: Vec<YouTubePlaylistTrackMetadata>,
}

impl YouTubePlaylistMetadata {
    pub fn can_edit(&self) -> bool {
        self.editable && self.owned && !self.playlist_id.trim().is_empty()
    }

    pub fn privacy_kind(&self) -> YouTubePlaylistPrivacy {
        match self.privacy.trim().to_ascii_uppercase().as_str() {
            "PRIVATE" => YouTubePlaylistPrivacy::Private,
            "UNLISTED" => YouTubePlaylistPrivacy::Unlisted,
            "PUBLIC" => YouTubePlaylistPrivacy::Public,
            _ => YouTubePlaylistPrivacy::Unknown,
        }
    }

    pub fn removable_track_count(&self) -> usize {
        self.tracks
            .iter()
            .filter(|track| track.has_removal_identity())
            .count()
    }

    pub fn has_unique_removal_identities(&self) -> bool {
        let mut identities = HashSet::new();
        self.tracks
            .iter()
            .filter(|track| track.has_removal_identity())
            .all(|track| identities.insert(track.set_video_id.trim()))
    }
}
