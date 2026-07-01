//! Durable cache-first snapshots for YouTube Music playlists.

use super::AppController;
use crate::{
    background::BackgroundMessage,
    youtube::{cache_items_for_browser, cacheable_youtube_playlist, YouTubeItem},
};
use gtk::glib;
use serde::{Deserialize, Serialize};
use std::{
    cell::{Cell, RefCell},
    collections::{hash_map::DefaultHasher, HashMap, HashSet, VecDeque},
    fs,
    hash::{Hash, Hasher},
    path::PathBuf,
    sync::{mpsc, Arc, Mutex},
    thread,
    time::{SystemTime, UNIX_EPOCH},
};

const PLAYLIST_SNAPSHOT_VERSION: u32 = 1;
const PLAYLIST_SNAPSHOT_REVALIDATE_SECS: u64 = 6 * 60 * 60;
const PLAYLIST_REVALIDATE_LIMIT: usize = 24;

thread_local! {
    static PENDING_REVALIDATION: RefCell<Vec<YouTubeItem>> = const { RefCell::new(Vec::new()) };
    static REVALIDATION_RUNNING: Cell<bool> = const { Cell::new(false) };
    static LAST_SNAPSHOT_SIGNATURE: Cell<u64> = const { Cell::new(0) };
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
struct PlaylistFirstPaintSnapshot {
    version: u32,
    saved_at: u64,
    playlists: Vec<YouTubeItem>,
    playlist_tracks: HashMap<String, Vec<YouTubeItem>>,
}

impl AppController {
    pub(crate) fn restore_playlist_first_paint_snapshot(&self) {
        if !youtube_library_cache_path().is_file() {
            clear_playlist_first_paint_snapshot();
            return;
        }

        let Some(snapshot) = load_playlist_first_paint_snapshot() else {
            return;
        };
        if snapshot.version != PLAYLIST_SNAPSHOT_VERSION {
            return;
        }

        let stale = snapshot_age_seconds(snapshot.saved_at) >= PLAYLIST_SNAPSHOT_REVALIDATE_SECS;
        let allowed = snapshot
            .playlists
            .iter()
            .filter(|playlist| cacheable_youtube_playlist(playlist))
            .map(|playlist| playlist.browse_id.clone())
            .collect::<HashSet<_>>();

        let mut restored_ids = HashSet::new();
        let mut library = self.youtube_library.borrow_mut();

        for playlist in snapshot
            .playlists
            .iter()
            .filter(|playlist| cacheable_youtube_playlist(playlist))
        {
            if library
                .playlists
                .iter()
                .all(|current| current.browse_id != playlist.browse_id)
            {
                library.playlists.push(playlist.clone());
            }
        }

        for (browse_id, items) in snapshot.playlist_tracks {
            if items.is_empty() || !allowed.contains(&browse_id) {
                continue;
            }
            if library
                .playlist_tracks
                .get(&browse_id)
                .is_some_and(|current| !current.is_empty())
            {
                continue;
            }
            library.playlist_tracks.insert(browse_id.clone(), items);
            restored_ids.insert(browse_id);
        }

        let restored = restored_ids.len();
        let playlists = if stale && restored > 0 {
            library
                .playlists
                .iter()
                .filter(|playlist| restored_ids.contains(&playlist.browse_id))
                .take(PLAYLIST_REVALIDATE_LIMIT)
                .cloned()
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        let signature = playlist_snapshot_signature(&library.playlists, &library.playlist_tracks);
        drop(library);

        LAST_SNAPSHOT_SIGNATURE.set(signature);
        PENDING_REVALIDATION.replace(playlists);

        if restored > 0 {
            eprintln!("Restored {restored} YouTube playlist snapshots for cache-first rendering");
        }
    }

    pub(crate) fn poll_playlist_snapshot_revalidation(&self) {
        let running = REVALIDATION_RUNNING.get();
        if running && !self.youtube_playlist_prefetching.get() {
            REVALIDATION_RUNNING.set(false);
            if matches!(
                self.browser.route(),
                crate::browser::BrowserRoute::YouTubePlaylist { .. }
            ) {
                self.refresh_browser();
            }
            return;
        }

        if running
            || self.youtube_playlist_prefetching.get()
            || !self.youtube_library.borrow().connected
        {
            return;
        }

        let playlists = PENDING_REVALIDATION.take();
        if playlists.is_empty() {
            return;
        }

        let Some(bridge) = self.youtube_bridge.clone() else {
            PENDING_REVALIDATION.replace(playlists);
            return;
        };

        self.youtube_playlist_prefetching.set(true);
        REVALIDATION_RUNNING.set(true);
        let sender = self.background.sender();

        thread::spawn(move || {
            let worker_count = playlists.len().min(3);
            let work = Arc::new(Mutex::new(playlists.into_iter().collect::<VecDeque<_>>()));
            let (result_tx, result_rx) = mpsc::channel();
            let mut workers = Vec::with_capacity(worker_count);

            for _ in 0..worker_count {
                let bridge = bridge.clone();
                let work = work.clone();
                let result_tx = result_tx.clone();
                workers.push(thread::spawn(move || loop {
                    let playlist = match work.lock() {
                        Ok(mut queue) => queue.pop_front(),
                        Err(_) => None,
                    };
                    let Some(playlist) = playlist else {
                        break;
                    };

                    let browse_id = playlist.browse_id.clone();
                    let result = bridge.playlist(&playlist).map(|mut items| {
                        cache_items_for_browser(&mut items);
                        items
                    });
                    let _ = result_tx.send((playlist.title, browse_id, result));
                }));
            }
            drop(result_tx);

            let mut refreshed = HashMap::new();
            for (title, browse_id, result) in result_rx {
                match result {
                    Ok(items) if !items.is_empty() => {
                        refreshed.insert(browse_id, items);
                    }
                    Ok(_) => {}
                    Err(error) => {
                        eprintln!("Could not revalidate cached playlist '{title}': {error}");
                    }
                }
            }
            for worker in workers {
                let _ = worker.join();
            }

            let _ = sender.send(BackgroundMessage::YouTubePlaylistsCached(Ok(refreshed)));
        });
    }

    pub(crate) fn checkpoint_playlist_first_paint_snapshot(&self) {
        let library_cache_exists = youtube_library_cache_path().is_file();
        let library = self.youtube_library.borrow();

        if !library_cache_exists && library.playlists.is_empty() && library.playlist_tracks.is_empty()
        {
            drop(library);
            clear_playlist_first_paint_snapshot();
            LAST_SNAPSHOT_SIGNATURE.set(0);
            PENDING_REVALIDATION.take();
            return;
        }

        if REVALIDATION_RUNNING.get() || !PENDING_REVALIDATION.with(|pending| pending.borrow().is_empty())
        {
            return;
        }

        let playlists = library
            .playlists
            .iter()
            .filter(|playlist| cacheable_youtube_playlist(playlist))
            .cloned()
            .collect::<Vec<_>>();
        let allowed = playlists
            .iter()
            .map(|playlist| playlist.browse_id.clone())
            .collect::<HashSet<_>>();
        let playlist_tracks = library
            .playlist_tracks
            .iter()
            .filter(|(browse_id, items)| allowed.contains(*browse_id) && !items.is_empty())
            .map(|(browse_id, items)| (browse_id.clone(), items.clone()))
            .collect::<HashMap<_, _>>();

        if playlist_tracks.is_empty() {
            return;
        }

        let signature = playlist_snapshot_signature(&playlists, &playlist_tracks);
        if LAST_SNAPSHOT_SIGNATURE.get() == signature {
            return;
        }
        LAST_SNAPSHOT_SIGNATURE.set(signature);
        drop(library);

        let snapshot = PlaylistFirstPaintSnapshot {
            version: PLAYLIST_SNAPSHOT_VERSION,
            saved_at: unix_now_seconds(),
            playlists,
            playlist_tracks,
        };
        thread::spawn(move || {
            if let Err(error) = save_playlist_first_paint_snapshot(&snapshot) {
                eprintln!("Could not save the playlist first-paint snapshot: {error}");
            }
        });
    }
}

fn load_playlist_first_paint_snapshot() -> Option<PlaylistFirstPaintSnapshot> {
    let raw = fs::read_to_string(playlist_first_paint_snapshot_path()).ok()?;
    serde_json::from_str(&raw).ok()
}

fn save_playlist_first_paint_snapshot(
    snapshot: &PlaylistFirstPaintSnapshot,
) -> Result<(), String> {
    let path = playlist_first_paint_snapshot_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Could not create playlist cache folder: {error}"))?;
    }
    let raw = serde_json::to_vec(snapshot)
        .map_err(|error| format!("Could not serialize playlist snapshot: {error}"))?;
    let temporary = path.with_extension("tmp");
    fs::write(&temporary, raw)
        .map_err(|error| format!("Could not write playlist snapshot: {error}"))?;
    fs::rename(&temporary, &path)
        .map_err(|error| format!("Could not publish playlist snapshot: {error}"))
}

fn clear_playlist_first_paint_snapshot() {
    let _ = fs::remove_file(playlist_first_paint_snapshot_path());
}

fn youtube_library_cache_path() -> PathBuf {
    glib::user_cache_dir()
        .join("nocky")
        .join("youtube")
        .join("library-cache.json")
}

fn playlist_first_paint_snapshot_path() -> PathBuf {
    glib::user_cache_dir()
        .join("nocky")
        .join("youtube")
        .join("playlist-first-paint-v1.json")
}

fn unix_now_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

fn snapshot_age_seconds(saved_at: u64) -> u64 {
    unix_now_seconds().saturating_sub(saved_at)
}

fn playlist_snapshot_signature(
    playlists: &[YouTubeItem],
    tracks: &HashMap<String, Vec<YouTubeItem>>,
) -> u64 {
    let mut hasher = DefaultHasher::new();
    for playlist in playlists {
        playlist.browse_id.hash(&mut hasher);
        playlist.title.hash(&mut hasher);
    }
    let mut browse_ids = tracks.keys().collect::<Vec<_>>();
    browse_ids.sort();
    for browse_id in browse_ids {
        browse_id.hash(&mut hasher);
        if let Some(items) = tracks.get(browse_id) {
            items.len().hash(&mut hasher);
            for item in items {
                item.video_id.hash(&mut hasher);
                item.title.hash(&mut hasher);
                item.cover_path.hash(&mut hasher);
            }
        }
    }
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn playlist(id: &str, kind: &str) -> YouTubeItem {
        YouTubeItem {
            result_type: "playlist".to_string(),
            title: format!("Playlist {id}"),
            browse_id: id.to_string(),
            playlist_kind: kind.to_string(),
            ..YouTubeItem::default()
        }
    }

    fn song(id: &str) -> YouTubeItem {
        YouTubeItem {
            result_type: "song".to_string(),
            title: format!("Song {id}"),
            video_id: id.to_string(),
            ..YouTubeItem::default()
        }
    }

    #[test]
    fn signature_changes_when_playlist_tracks_change() {
        let playlists = vec![playlist("PL1", "library")];
        let mut tracks = HashMap::from([("PL1".to_string(), vec![song("one")])]);
        let first = playlist_snapshot_signature(&playlists, &tracks);
        tracks.get_mut("PL1").unwrap().push(song("two"));
        assert_ne!(first, playlist_snapshot_signature(&playlists, &tracks));
    }

    #[test]
    fn cacheable_contract_excludes_temporary_recommendations() {
        assert!(cacheable_youtube_playlist(&playlist("PL1", "library")));
        assert!(!cacheable_youtube_playlist(&playlist("PL2", "mix")));
        assert!(!cacheable_youtube_playlist(&playlist("PL3", "recommended")));
    }
}
