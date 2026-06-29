//! Read-only YouTube playlist metadata integration.
//!
//! Metadata is loaded on a worker thread through the sanitized packaged helper.
//! The main thread receives only a display-safe diagnostic string. No raw
//! service response is persisted and no playlist mutation is exposed here.

use super::AppController;
use crate::{
    browser::BrowserRoute,
    youtube::{cacheable_youtube_playlist, YouTubeItem},
};
use std::{
    collections::HashSet,
    sync::{
        mpsc::{self, Receiver, Sender, TryRecvError},
        Mutex, OnceLock,
    },
    thread,
};

type PlaylistMetadataResult = (String, Result<String, String>);

fn metadata_channel() -> &'static (
    Sender<PlaylistMetadataResult>,
    Mutex<Receiver<PlaylistMetadataResult>>,
) {
    static CHANNEL: OnceLock<(
        Sender<PlaylistMetadataResult>,
        Mutex<Receiver<PlaylistMetadataResult>>,
    )> = OnceLock::new();

    CHANNEL.get_or_init(|| {
        let (sender, receiver) = mpsc::channel();
        (sender, Mutex::new(receiver))
    })
}

fn pending_metadata_requests() -> &'static Mutex<HashSet<String>> {
    static PENDING: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    PENDING.get_or_init(|| Mutex::new(HashSet::new()))
}

fn completed_metadata_requests() -> &'static Mutex<HashSet<String>> {
    static COMPLETED: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    COMPLETED.get_or_init(|| Mutex::new(HashSet::new()))
}

fn mark_metadata_pending(
    pending: &mut HashSet<String>,
    completed: &HashSet<String>,
    browse_id: &str,
) -> bool {
    let browse_id = browse_id.trim();
    !browse_id.is_empty()
        && !completed.contains(browse_id)
        && pending.insert(browse_id.to_string())
}

fn apply_playlist_metadata_diagnostic(
    playlists: &mut [YouTubeItem],
    browse_id: &str,
    diagnostic: &str,
) -> bool {
    let browse_id = browse_id.trim();
    if browse_id.is_empty() || diagnostic.trim().is_empty() {
        return false;
    }

    let Some(playlist) = playlists
        .iter_mut()
        .find(|playlist| playlist.browse_id.trim() == browse_id)
    else {
        return false;
    };

    if playlist.subtitle == diagnostic {
        return false;
    }

    playlist.subtitle = diagnostic.to_string();
    true
}

impl AppController {
    pub(crate) fn poll_current_youtube_playlist_metadata(&self) {
        let browse_id = match self.browser.route() {
            BrowserRoute::YouTubePlaylist { browse_id, .. } => browse_id,
            _ => return,
        };
        let playlist = self
            .youtube_library
            .borrow()
            .playlists
            .iter()
            .find(|playlist| playlist.browse_id == browse_id)
            .cloned();

        if let Some(playlist) = playlist {
            self.request_youtube_playlist_metadata(playlist);
        }
    }

    pub(crate) fn request_youtube_playlist_metadata(&self, playlist: YouTubeItem) {
        if !self.youtube_library.borrow().connected
            || !cacheable_youtube_playlist(&playlist)
            || playlist.browse_id.trim().is_empty()
        {
            return;
        }

        let Some(bridge) = self.youtube_bridge.clone() else {
            return;
        };
        let browse_id = playlist.browse_id.trim().to_string();

        {
            let Ok(mut pending) = pending_metadata_requests().lock() else {
                return;
            };
            let Ok(completed) = completed_metadata_requests().lock() else {
                return;
            };
            if !mark_metadata_pending(&mut pending, &completed, &browse_id) {
                return;
            }
        }

        let sender = metadata_channel().0.clone();
        thread::spawn(move || {
            let result = bridge.playlist_metadata_diagnostic(&browse_id);
            let _ = sender.send((browse_id, result));
        });
    }

    pub(crate) fn handle_youtube_playlist_metadata_updates(&self) {
        let Ok(receiver) = metadata_channel().1.lock() else {
            return;
        };

        loop {
            let update = match receiver.try_recv() {
                Ok(update) => update,
                Err(TryRecvError::Empty | TryRecvError::Disconnected) => break,
            };
            let (browse_id, result) = update;

            if let Ok(mut pending) = pending_metadata_requests().lock() {
                pending.remove(&browse_id);
            }
            if let Ok(mut completed) = completed_metadata_requests().lock() {
                completed.insert(browse_id.clone());
            }

            match result {
                Ok(diagnostic) => {
                    let changed = apply_playlist_metadata_diagnostic(
                        &mut self.youtube_library.borrow_mut().playlists,
                        &browse_id,
                        &diagnostic,
                    );
                    if changed && self.is_open_youtube_playlist(&browse_id) {
                        self.refresh_browser();
                    }
                }
                Err(error) => {
                    eprintln!(
                        "Could not load read-only YouTube playlist metadata for {browse_id}: {error}"
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{apply_playlist_metadata_diagnostic, mark_metadata_pending};
    use crate::youtube::YouTubeItem;
    use std::collections::HashSet;

    #[test]
    fn pending_requests_reject_empty_duplicate_and_completed_ids() {
        let mut pending = HashSet::new();
        let mut completed = HashSet::new();

        assert!(!mark_metadata_pending(&mut pending, &completed, "  "));
        assert!(mark_metadata_pending(
            &mut pending,
            &completed,
            "PL-owned"
        ));
        assert!(!mark_metadata_pending(
            &mut pending,
            &completed,
            "PL-owned"
        ));
        pending.remove("PL-owned");
        completed.insert("PL-owned".to_string());
        assert!(!mark_metadata_pending(
            &mut pending,
            &completed,
            "PL-owned"
        ));
    }

    #[test]
    fn diagnostic_updates_only_the_matching_playlist() {
        let mut playlists = vec![
            YouTubeItem {
                result_type: "playlist".to_string(),
                title: "First".to_string(),
                subtitle: "Old first".to_string(),
                browse_id: "PL-first".to_string(),
                ..YouTubeItem::default()
            },
            YouTubeItem {
                result_type: "playlist".to_string(),
                title: "Second".to_string(),
                subtitle: "Old second".to_string(),
                browse_id: "PL-second".to_string(),
                ..YouTubeItem::default()
            },
        ];

        assert!(apply_playlist_metadata_diagnostic(
            &mut playlists,
            "PL-second",
            "Playlist própria • privada • 2 ocorrências identificadas",
        ));
        assert_eq!(playlists[0].subtitle, "Old first");
        assert_eq!(
            playlists[1].subtitle,
            "Playlist própria • privada • 2 ocorrências identificadas"
        );
    }

    #[test]
    fn missing_or_unchanged_diagnostic_does_not_report_change() {
        let mut playlists = vec![YouTubeItem {
            browse_id: "PL-owned".to_string(),
            subtitle: "Playlist própria • privada".to_string(),
            ..YouTubeItem::default()
        }];

        assert!(!apply_playlist_metadata_diagnostic(
            &mut playlists,
            "PL-missing",
            "Playlist compartilhada",
        ));
        assert!(!apply_playlist_metadata_diagnostic(
            &mut playlists,
            "PL-owned",
            "Playlist própria • privada",
        ));
        assert!(!apply_playlist_metadata_diagnostic(
            &mut playlists,
            "PL-owned",
            "  ",
        ));
    }
}
