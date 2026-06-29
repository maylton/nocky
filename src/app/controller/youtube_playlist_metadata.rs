//! Read-only YouTube playlist metadata integration.
//!
//! Metadata is loaded on a worker thread through the sanitized packaged helper.
//! Native code retains only the display diagnostic and the confirmed editability
//! bit needed by the separately reviewed playlist-item action.

#[path = "youtube_playlist_add.rs"]
mod playlist_add;

use super::AppController;
use crate::{
    browser::BrowserRoute,
    youtube::{cacheable_youtube_playlist, YouTubeItem},
};
use std::{
    collections::{HashMap, HashSet},
    sync::{
        mpsc::{self, Receiver, Sender},
        Mutex, OnceLock,
    },
    thread,
};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct PlaylistMetadataAccess {
    diagnostic: String,
    editable: bool,
}

type PlaylistMetadataResult = (String, Result<PlaylistMetadataAccess, String>);

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

fn cached_metadata_results() -> &'static Mutex<HashMap<String, Option<PlaylistMetadataAccess>>> {
    static RESULTS: OnceLock<Mutex<HashMap<String, Option<PlaylistMetadataAccess>>>> =
        OnceLock::new();
    RESULTS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn mark_metadata_pending(pending: &mut HashSet<String>, browse_id: &str) -> bool {
    let browse_id = browse_id.trim();
    !browse_id.is_empty() && pending.insert(browse_id.to_string())
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

        let browse_id = playlist.browse_id.trim().to_string();
        let cached = cached_metadata_results()
            .lock()
            .ok()
            .and_then(|results| results.get(&browse_id).cloned());
        match cached {
            Some(Some(access)) => {
                let changed = apply_playlist_metadata_diagnostic(
                    &mut self.youtube_library.borrow_mut().playlists,
                    &browse_id,
                    &access.diagnostic,
                );
                if changed && self.is_open_youtube_playlist(&browse_id) {
                    self.refresh_browser();
                }
                self.update_youtube_playlist_add_action();
                return;
            }
            Some(None) => {
                self.update_youtube_playlist_add_action();
                return;
            }
            None => {}
        }

        let Some(bridge) = self.youtube_bridge.clone() else {
            return;
        };
        {
            let Ok(mut pending) = pending_metadata_requests().lock() else {
                return;
            };
            if !mark_metadata_pending(&mut pending, &browse_id) {
                return;
            }
        }

        let sender = metadata_channel().0.clone();
        thread::spawn(move || {
            let result = bridge
                .playlist_metadata_access(&browse_id)
                .map(|(diagnostic, editable)| PlaylistMetadataAccess {
                    diagnostic,
                    editable,
                });
            let _ = sender.send((browse_id, result));
        });
    }

    pub(crate) fn cached_youtube_playlist_editability(&self, browse_id: &str) -> Option<bool> {
        cached_metadata_results()
            .lock()
            .ok()
            .and_then(|results| results.get(browse_id.trim()).cloned().flatten())
            .map(|access| access.editable)
    }

    pub(crate) fn invalidate_youtube_playlist_metadata(&self, browse_id: &str) {
        if let Ok(mut results) = cached_metadata_results().lock() {
            results.remove(browse_id.trim());
        }
    }

    pub(crate) fn handle_youtube_playlist_metadata_updates(&self) {
        let Ok(receiver) = metadata_channel().1.lock() else {
            return;
        };
        while let Ok((browse_id, result)) = receiver.try_recv() {
            if let Ok(mut pending) = pending_metadata_requests().lock() {
                pending.remove(&browse_id);
            }
            match result {
                Ok(access) => {
                    if let Ok(mut results) = cached_metadata_results().lock() {
                        results.insert(browse_id.clone(), Some(access.clone()));
                    }
                    let changed = apply_playlist_metadata_diagnostic(
                        &mut self.youtube_library.borrow_mut().playlists,
                        &browse_id,
                        &access.diagnostic,
                    );
                    if changed && self.is_open_youtube_playlist(&browse_id) {
                        self.refresh_browser();
                    }
                }
                Err(error) => {
                    if let Ok(mut results) = cached_metadata_results().lock() {
                        results.insert(browse_id.clone(), None);
                    }
                    eprintln!(
                        "Could not load read-only YouTube playlist metadata for {browse_id}: {error}"
                    );
                }
            }
            self.update_youtube_playlist_add_action();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{apply_playlist_metadata_diagnostic, mark_metadata_pending};
    use crate::youtube::YouTubeItem;
    use std::collections::HashSet;

    #[test]
    fn pending_requests_reject_empty_and_duplicate_ids() {
        let mut pending = HashSet::new();
        assert!(!mark_metadata_pending(&mut pending, "  "));
        assert!(mark_metadata_pending(&mut pending, "PL-owned"));
        assert!(!mark_metadata_pending(&mut pending, "PL-owned"));
    }

    #[test]
    fn diagnostic_updates_only_the_matching_playlist() {
        let mut playlists = vec![
            YouTubeItem {
                browse_id: "PL-first".to_string(),
                subtitle: "Old first".to_string(),
                ..YouTubeItem::default()
            },
            YouTubeItem {
                browse_id: "PL-second".to_string(),
                subtitle: "Old second".to_string(),
                ..YouTubeItem::default()
            },
        ];
        assert!(apply_playlist_metadata_diagnostic(
            &mut playlists,
            "PL-second",
            "Playlist própria • privada"
        ));
        assert_eq!(playlists[0].subtitle, "Old first");
        assert_eq!(playlists[1].subtitle, "Playlist própria • privada");
    }
}
