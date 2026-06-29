#![allow(dead_code)]

use std::collections::HashSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlaylistPrivacy {
    Private,
    Unlisted,
    Public,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlaylistMutationRisk {
    NonDestructive,
    Reversible,
    Destructive,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PlaylistTarget {
    pub playlist_id: String,
    pub title: String,
    pub owned: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PlaylistTrackIdentity {
    pub video_id: String,
    pub set_video_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlaylistMutationRequest {
    Create {
        title: String,
        description: String,
        privacy: PlaylistPrivacy,
    },
    AddTracks {
        target: PlaylistTarget,
        video_ids: Vec<String>,
    },
    EditMetadata {
        target: PlaylistTarget,
        title: Option<String>,
        description: Option<String>,
        privacy: Option<PlaylistPrivacy>,
    },
    RemoveTracks {
        target: PlaylistTarget,
        tracks: Vec<PlaylistTrackIdentity>,
    },
    Delete {
        target: PlaylistTarget,
        confirmation: String,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlaylistMutationBlock {
    MissingPlaylistId,
    MissingTitle,
    InvalidTitle,
    NotOwned,
    MissingVideoId,
    MissingSetVideoId,
    DuplicateVideoId,
    NoChanges,
    ConfirmationMismatch,
}

impl PlaylistMutationRequest {
    pub fn risk(&self) -> PlaylistMutationRisk {
        match self {
            Self::Create { .. } => PlaylistMutationRisk::NonDestructive,
            Self::AddTracks { .. } | Self::EditMetadata { .. } => {
                PlaylistMutationRisk::Reversible
            }
            Self::RemoveTracks { .. } | Self::Delete { .. } => {
                PlaylistMutationRisk::Destructive
            }
        }
    }

    pub fn validate(&self) -> Result<(), Vec<PlaylistMutationBlock>> {
        let mut blocks = Vec::new();

        match self {
            Self::Create { title, .. } => validate_title(title, &mut blocks),
            Self::AddTracks { target, video_ids } => {
                validate_owned_target(target, &mut blocks);
                if video_ids.is_empty() {
                    blocks.push(PlaylistMutationBlock::MissingVideoId);
                }

                let mut seen = HashSet::new();
                for video_id in video_ids {
                    let normalized = video_id.trim();
                    if normalized.is_empty() {
                        blocks.push(PlaylistMutationBlock::MissingVideoId);
                    } else if !seen.insert(normalized) {
                        blocks.push(PlaylistMutationBlock::DuplicateVideoId);
                    }
                }
            }
            Self::EditMetadata {
                target,
                title,
                description,
                privacy,
            } => {
                validate_owned_target(target, &mut blocks);
                if let Some(title) = title {
                    validate_title(title, &mut blocks);
                }
                if title.is_none() && description.is_none() && privacy.is_none() {
                    blocks.push(PlaylistMutationBlock::NoChanges);
                }
            }
            Self::RemoveTracks { target, tracks } => {
                validate_owned_target(target, &mut blocks);
                if tracks.is_empty() {
                    blocks.push(PlaylistMutationBlock::MissingVideoId);
                }

                let mut seen = HashSet::new();
                for track in tracks {
                    let video_id = track.video_id.trim();
                    let set_video_id = track.set_video_id.trim();
                    if video_id.is_empty() {
                        blocks.push(PlaylistMutationBlock::MissingVideoId);
                    } else if !seen.insert(video_id) {
                        blocks.push(PlaylistMutationBlock::DuplicateVideoId);
                    }
                    if set_video_id.is_empty() {
                        blocks.push(PlaylistMutationBlock::MissingSetVideoId);
                    }
                }
            }
            Self::Delete {
                target,
                confirmation,
            } => {
                validate_owned_target(target, &mut blocks);
                if target.title.trim().is_empty()
                    || confirmation.trim() != target.title.trim()
                {
                    blocks.push(PlaylistMutationBlock::ConfirmationMismatch);
                }
            }
        }

        blocks.sort_by_key(|block| *block as u8);
        blocks.dedup();
        if blocks.is_empty() {
            Ok(())
        } else {
            Err(blocks)
        }
    }
}

fn validate_title(title: &str, blocks: &mut Vec<PlaylistMutationBlock>) {
    let title = title.trim();
    if title.is_empty() {
        blocks.push(PlaylistMutationBlock::MissingTitle);
    }
    if title.contains('<') || title.contains('>') {
        blocks.push(PlaylistMutationBlock::InvalidTitle);
    }
}

fn validate_owned_target(target: &PlaylistTarget, blocks: &mut Vec<PlaylistMutationBlock>) {
    if target.playlist_id.trim().is_empty() {
        blocks.push(PlaylistMutationBlock::MissingPlaylistId);
    }
    if !target.owned {
        blocks.push(PlaylistMutationBlock::NotOwned);
    }
}
