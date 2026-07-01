use crate::{app::controller::AppController, youtube::YouTubeItem};
use gtk::glib;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs, path::PathBuf};

const CACHE_VERSION: u32 = 1;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
struct PersistedOpenedPlaylistCache {
    version: u32,
    playlists: HashMap<String, Vec<YouTubeItem>>,
}

pub(super) struct DurablePlaylistCache {
    path: PathBuf,
    playlists: HashMap<String, Vec<YouTubeItem>>,
}

impl DurablePlaylistCache {
    pub(super) fn load(controller: &AppController) -> Self {
        let path = cache_path();
        let playlists = fs::read_to_string(&path)
            .ok()
            .map(|raw| decode(&raw))
            .unwrap_or_default();

        {
            let mut library = controller.youtube_library.borrow_mut();
            for (browse_id, items) in &playlists {
                let current = library.playlist_tracks.entry(browse_id.clone()).or_default();
                if current.is_empty() {
                    *current = items.clone();
                }
            }
        }

        Self { path, playlists }
    }

    pub(super) fn items_with_fallback(
        &self,
        controller: &AppController,
        browse_id: &str,
    ) -> (Vec<YouTubeItem>, bool) {
        if let Some(items) = controller
            .youtube_library
            .borrow()
            .playlist_tracks
            .get(browse_id)
            .filter(|items| !items.is_empty())
            .cloned()
        {
            return (items, false);
        }

        let Some(items) = self
            .playlists
            .get(browse_id)
            .filter(|items| !items.is_empty())
            .cloned()
        else {
            return (Vec::new(), false);
        };

        controller
            .youtube_library
            .borrow_mut()
            .playlist_tracks
            .insert(browse_id.to_string(), items.clone());
        (items, true)
    }

    pub(super) fn persist_if_changed(&mut self, browse_id: &str, items: &[YouTubeItem]) {
        if self.playlists.get(browse_id).map(Vec::as_slice) == Some(items) {
            return;
        }

        let mut next = self.playlists.clone();
        next.insert(browse_id.to_string(), items.to_vec());
        next = sanitize(next);

        match save(&self.path, &next) {
            Ok(()) => self.playlists = next,
            Err(error) => {
                eprintln!("Could not persist opened YouTube playlist {browse_id}: {error}")
            }
        }
    }
}

fn cache_path() -> PathBuf {
    glib::user_cache_dir()
        .join("nocky")
        .join("youtube")
        .join("opened-playlists-v1.json")
}

fn decode(raw: &str) -> HashMap<String, Vec<YouTubeItem>> {
    let Ok(cache) = serde_json::from_str::<PersistedOpenedPlaylistCache>(raw) else {
        eprintln!("Ignoring corrupt opened-playlist cache");
        return HashMap::new();
    };
    if cache.version != CACHE_VERSION {
        eprintln!(
            "Ignoring incompatible opened-playlist cache version {}",
            cache.version
        );
        return HashMap::new();
    }

    sanitize(cache.playlists)
}

fn sanitize(
    playlists: HashMap<String, Vec<YouTubeItem>>,
) -> HashMap<String, Vec<YouTubeItem>> {
    playlists
        .into_iter()
        .filter_map(|(browse_id, items)| {
            let browse_id = browse_id.trim().to_string();
            if browse_id.is_empty() {
                return None;
            }

            let items = items
                .into_iter()
                .filter(YouTubeItem::playable)
                .collect::<Vec<_>>();
            (!items.is_empty()).then_some((browse_id, items))
        })
        .collect()
}

fn save(path: &std::path::Path, playlists: &HashMap<String, Vec<YouTubeItem>>) -> Result<(), String> {
    let Some(parent) = path.parent() else {
        return Err("Opened-playlist cache path has no parent directory".to_string());
    };
    fs::create_dir_all(parent)
        .map_err(|error| format!("Could not create opened-playlist cache folder: {error}"))?;

    let payload = PersistedOpenedPlaylistCache {
        version: CACHE_VERSION,
        playlists: playlists.clone(),
    };
    let serialized = serde_json::to_vec(&payload)
        .map_err(|error| format!("Could not serialize opened-playlist cache: {error}"))?;
    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, serialized)
        .map_err(|error| format!("Could not write opened-playlist cache: {error}"))?;
    fs::rename(&temporary, path)
        .map_err(|error| format!("Could not replace opened-playlist cache: {error}"))
}

#[cfg(test)]
mod tests {
    use super::{decode, sanitize, PersistedOpenedPlaylistCache, CACHE_VERSION};
    use crate::youtube::YouTubeItem;
    use std::collections::HashMap;

    fn track(video_id: &str) -> YouTubeItem {
        YouTubeItem {
            result_type: "song".to_string(),
            video_id: video_id.to_string(),
            title: "Cached track".to_string(),
            ..YouTubeItem::default()
        }
    }

    #[test]
    fn keeps_tracks_for_playlist_outside_the_library_list() {
        let payload = PersistedOpenedPlaylistCache {
            version: CACHE_VERSION,
            playlists: HashMap::from([(
                "VLrecommended".to_string(),
                vec![track("video-1"), track("video-2")],
            )]),
        };
        let raw = serde_json::to_string(&payload).unwrap();

        assert_eq!(decode(&raw)["VLrecommended"].len(), 2);
    }

    #[test]
    fn drops_empty_ids_and_empty_track_lists() {
        let sanitized = sanitize(HashMap::from([
            ("".to_string(), vec![track("video-1")]),
            ("VLempty".to_string(), Vec::new()),
            ("VLvalid".to_string(), vec![track("video-2")]),
        ]));

        assert_eq!(sanitized.len(), 1);
        assert_eq!(sanitized["VLvalid"][0].video_id, "video-2");
    }

    #[test]
    fn rejects_incompatible_cache_version() {
        let payload = PersistedOpenedPlaylistCache {
            version: CACHE_VERSION + 1,
            playlists: HashMap::from([("VLcached".to_string(), vec![track("video-1")])]),
        };
        let raw = serde_json::to_string(&payload).unwrap();

        assert!(decode(&raw).is_empty());
    }
}
