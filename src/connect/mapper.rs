use std::{fs, path::Path, time::UNIX_EPOCH};

use crate::playback::queue::{PlaybackQueue, QueueMedia, QueueSource, QueueSourceKind};

use super::protocol::{
    LocalTrackIdentity, NockyConnectSource, NockyPlaybackState, NockyRepeatMode, PlaybackInfo,
    PlaybackSessionSnapshot, PortableAlbum, PortableArtist, PortableQueue, PortableQueueItem,
};

#[derive(Clone, Debug, PartialEq)]
pub struct DesktopPlaybackState {
    pub state: NockyPlaybackState,
    pub position_ms: u64,
    pub duration_ms: Option<u64>,
    pub rate: f32,
    pub volume: Option<f32>,
    pub muted: bool,
    pub repeat_mode: NockyRepeatMode,
    pub shuffle_enabled: bool,
}

impl Default for DesktopPlaybackState {
    fn default() -> Self {
        Self {
            state: NockyPlaybackState::Paused,
            position_ms: 0,
            duration_ms: None,
            rate: 1.0,
            volume: None,
            muted: false,
            repeat_mode: NockyRepeatMode::Off,
            shuffle_enabled: false,
        }
    }
}

impl DesktopPlaybackState {
    fn playback_info(&self, fallback_duration_ms: Option<u64>) -> PlaybackInfo {
        PlaybackInfo {
            state: self.state,
            position_ms: self.position_ms,
            duration_ms: self.duration_ms.or(fallback_duration_ms),
            rate: self.rate,
            volume: self.volume,
            muted: self.muted,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct RestoredDesktopSnapshot {
    pub queue: PlaybackQueue,
    pub state: DesktopPlaybackState,
    pub title: Option<String>,
}

pub fn export_desktop_queue_snapshot(
    queue: &PlaybackQueue,
    title: Option<String>,
    playback_state: DesktopPlaybackState,
    session_id: impl Into<String>,
    revision: u64,
    origin_device_id: impl Into<String>,
    updated_at_epoch_ms: u64,
) -> PlaybackSessionSnapshot {
    let source = queue
        .source_kind()
        .ok()
        .flatten()
        .map(connect_source_from_queue_source_kind)
        .unwrap_or(NockyConnectSource::Unknown);
    let current_index = queue.current_index().unwrap_or(0);
    let fallback_duration_ms = queue
        .current()
        .and_then(|entry| duration_ms(entry.media.duration_seconds));

    PlaybackSessionSnapshot::new(
        session_id,
        revision,
        origin_device_id,
        updated_at_epoch_ms,
        source,
        playback_state.playback_info(fallback_duration_ms),
        PortableQueue {
            title,
            current_index,
            repeat_mode: playback_state.repeat_mode,
            shuffle_enabled: playback_state.shuffle_enabled,
            shuffle_seed: None,
            items: queue
                .entries()
                .iter()
                .map(|entry| portable_item_from_media(&entry.media))
                .collect(),
        },
    )
}

pub fn restore_desktop_queue_snapshot(
    snapshot: &PlaybackSessionSnapshot,
) -> RestoredDesktopSnapshot {
    let media = snapshot
        .queue
        .items
        .iter()
        .map(queue_media_from_portable_item)
        .collect::<Vec<_>>();
    let mut queue = PlaybackQueue::new();
    let current_index = if media.is_empty() {
        None
    } else {
        Some(snapshot.queue.current_index.min(media.len().saturating_sub(1)))
    };
    queue.replace(media, current_index);

    RestoredDesktopSnapshot {
        queue,
        title: snapshot.queue.title.clone(),
        state: DesktopPlaybackState {
            state: NockyPlaybackState::Paused,
            position_ms: snapshot.playback.position_ms,
            duration_ms: snapshot.playback.duration_ms,
            rate: snapshot.playback.rate,
            volume: snapshot.playback.volume,
            muted: snapshot.playback.muted,
            repeat_mode: snapshot.queue.repeat_mode,
            shuffle_enabled: snapshot.queue.shuffle_enabled,
        },
    }
}

fn portable_item_from_media(media: &QueueMedia) -> PortableQueueItem {
    match &media.source {
        QueueSource::YouTube { video_id } => PortableQueueItem {
            queue_item_id: format!("youtube:video:{video_id}"),
            source: NockyConnectSource::YouTube,
            provider: "youtube_music".to_string(),
            playable_id: video_id.clone(),
            set_video_id: None,
            playlist_id: None,
            browse_id: None,
            title: media.title.clone(),
            artists: vec![PortableArtist {
                id: None,
                name: media.artist.clone(),
            }],
            album: Some(PortableAlbum {
                id: None,
                title: media.album.clone(),
            }),
            duration_ms: duration_ms(media.duration_seconds),
            thumbnail_url: media
                .cover_path
                .as_ref()
                .map(|path| path.to_string_lossy().to_string()),
            explicit: false,
            is_video: false,
            is_episode: false,
            local: None,
        },
        QueueSource::Local { path } => PortableQueueItem {
            queue_item_id: format!("local:{}", path.to_string_lossy()),
            source: NockyConnectSource::Local,
            provider: "nocky_local".to_string(),
            playable_id: path.to_string_lossy().to_string(),
            set_video_id: None,
            playlist_id: None,
            browse_id: None,
            title: media.title.clone(),
            artists: vec![PortableArtist {
                id: None,
                name: media.artist.clone(),
            }],
            album: Some(PortableAlbum {
                id: None,
                title: media.album.clone(),
            }),
            duration_ms: duration_ms(media.duration_seconds),
            thumbnail_url: media
                .cover_path
                .as_ref()
                .map(|path| path.to_string_lossy().to_string()),
            explicit: false,
            is_video: false,
            is_episode: false,
            local: Some(local_identity_for_path(path)),
        },
    }
}

fn queue_media_from_portable_item(item: &PortableQueueItem) -> QueueMedia {
    let artist = item
        .artists
        .first()
        .map(|artist| artist.name.clone())
        .unwrap_or_else(|| "Unknown artist".to_string());
    let album = item
        .album
        .as_ref()
        .map(|album| album.title.clone())
        .unwrap_or_else(|| "Unknown album".to_string());
    let duration_seconds = item.duration_ms.unwrap_or(0) / 1_000;
    let cover_path = item.thumbnail_url.as_ref().map(std::path::PathBuf::from);

    match item.source {
        NockyConnectSource::Local => QueueMedia::local(
            std::path::PathBuf::from(&item.playable_id),
            item.title.clone(),
            artist,
            album,
            duration_seconds,
            cover_path,
        ),
        NockyConnectSource::YouTube | NockyConnectSource::Unknown => QueueMedia::youtube(
            item.playable_id.clone(),
            item.title.clone(),
            artist,
            album,
            duration_seconds,
            cover_path,
        ),
    }
}

fn connect_source_from_queue_source_kind(kind: QueueSourceKind) -> NockyConnectSource {
    match kind {
        QueueSourceKind::Local => NockyConnectSource::Local,
        QueueSourceKind::YouTube => NockyConnectSource::YouTube,
    }
}

fn duration_ms(duration_seconds: u64) -> Option<u64> {
    (duration_seconds > 0).then_some(duration_seconds.saturating_mul(1_000))
}

fn local_identity_for_path(path: &Path) -> LocalTrackIdentity {
    let metadata = fs::metadata(path).ok();
    let modified_at_epoch_ms = metadata
        .as_ref()
        .and_then(|metadata| metadata.modified().ok())
        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64);

    LocalTrackIdentity {
        library_id: Some("desktop-local-library".to_string()),
        content_hash: None,
        relative_path: Some(path.to_string_lossy().to_string()),
        file_size: metadata.as_ref().map(|metadata| metadata.len()),
        modified_at_epoch_ms,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::playback::queue::QueueMedia;

    #[test]
    fn exports_youtube_queue_snapshot() {
        let mut queue = PlaybackQueue::new();
        queue.replace(
            vec![
                QueueMedia::youtube("video-1", "First", "Artist One", "Album", 180, None),
                QueueMedia::youtube("video-2", "Second", "Artist Two", "Album", 181, None),
            ],
            Some(1),
        );

        let snapshot = export_desktop_queue_snapshot(
            &queue,
            Some("Desktop queue".to_string()),
            DesktopPlaybackState {
                position_ms: 42_000,
                repeat_mode: NockyRepeatMode::All,
                shuffle_enabled: true,
                ..Default::default()
            },
            "desktop-session",
            2,
            "desktop-device",
            1_700_000_000_000,
        );

        assert_eq!(snapshot.source, NockyConnectSource::YouTube);
        assert_eq!(snapshot.queue.current_index, 1);
        assert_eq!(snapshot.queue.items.len(), 2);
        assert_eq!(snapshot.queue.items[1].queue_item_id, "youtube:video:video-2");
        assert_eq!(snapshot.queue.items[1].playable_id, "video-2");
        assert_eq!(snapshot.playback.position_ms, 42_000);
        assert_eq!(snapshot.playback.duration_ms, Some(181_000));
        assert_eq!(snapshot.queue.repeat_mode, NockyRepeatMode::All);
        assert!(snapshot.queue.shuffle_enabled);
    }

    #[test]
    fn restores_snapshot_as_paused_queue() {
        let snapshot = PlaybackSessionSnapshot::new(
            "restore-session",
            1,
            "android-device",
            1_700_000_000_000,
            NockyConnectSource::YouTube,
            PlaybackInfo {
                state: NockyPlaybackState::Playing,
                position_ms: 90_000,
                duration_ms: Some(180_000),
                rate: 1.0,
                volume: Some(0.7),
                muted: false,
            },
            PortableQueue {
                title: Some("Remote".to_string()),
                current_index: 9,
                repeat_mode: NockyRepeatMode::One,
                shuffle_enabled: true,
                shuffle_seed: None,
                items: vec![PortableQueueItem {
                    queue_item_id: "youtube:video:video-1".to_string(),
                    source: NockyConnectSource::YouTube,
                    provider: "youtube_music".to_string(),
                    playable_id: "video-1".to_string(),
                    set_video_id: None,
                    playlist_id: None,
                    browse_id: None,
                    title: "First".to_string(),
                    artists: vec![PortableArtist {
                        id: None,
                        name: "Artist One".to_string(),
                    }],
                    album: Some(PortableAlbum {
                        id: None,
                        title: "Album".to_string(),
                    }),
                    duration_ms: Some(180_000),
                    thumbnail_url: None,
                    explicit: false,
                    is_video: false,
                    is_episode: false,
                    local: None,
                }],
            },
        );

        let restored = restore_desktop_queue_snapshot(&snapshot);

        assert_eq!(restored.queue.len(), 1);
        assert_eq!(restored.queue.current_index(), Some(0));
        assert_eq!(restored.state.state, NockyPlaybackState::Paused);
        assert_eq!(restored.state.position_ms, 90_000);
        assert_eq!(restored.state.repeat_mode, NockyRepeatMode::One);
        assert!(restored.state.shuffle_enabled);
    }
}
