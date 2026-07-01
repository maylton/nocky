use super::YouTubeHomePage;
use gtk::glib;
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

const HOME_SNAPSHOT_VERSION: u32 = 1;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
struct PersistedYouTubeHomeSnapshot {
    version: u32,
    page: YouTubeHomePage,
}

pub(crate) fn load_youtube_home_snapshot() -> YouTubeHomePage {
    let path = youtube_home_snapshot_path();
    let Ok(raw) = fs::read_to_string(path) else {
        return YouTubeHomePage::default();
    };

    decode_youtube_home_snapshot(&raw).unwrap_or_default()
}

pub(crate) fn save_youtube_home_snapshot(page: &YouTubeHomePage) -> Result<(), String> {
    let Some(snapshot) = snapshot_from_page(page) else {
        return Ok(());
    };

    let path = youtube_home_snapshot_path();
    let Some(parent) = path.parent() else {
        return Err("YouTube Home snapshot path has no parent directory".to_string());
    };
    fs::create_dir_all(parent)
        .map_err(|error| format!("Could not create YouTube Home snapshot folder: {error}"))?;

    let raw = serde_json::to_vec(&snapshot)
        .map_err(|error| format!("Could not serialize YouTube Home snapshot: {error}"))?;
    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, raw)
        .map_err(|error| format!("Could not write YouTube Home snapshot: {error}"))?;
    fs::rename(&temporary, &path)
        .map_err(|error| format!("Could not replace YouTube Home snapshot: {error}"))
}

pub(crate) fn clear_youtube_home_snapshot() {
    let _ = fs::remove_file(youtube_home_snapshot_path());
}

fn youtube_home_snapshot_path() -> PathBuf {
    glib::user_cache_dir()
        .join("nocky")
        .join("youtube")
        .join("home-snapshot-v1.json")
}

fn snapshot_from_page(page: &YouTubeHomePage) -> Option<PersistedYouTubeHomeSnapshot> {
    let mut page = page.clone();
    page.sections.retain(|section| !section.items.is_empty());
    if page.sections.is_empty() {
        return None;
    }

    Some(PersistedYouTubeHomeSnapshot {
        version: HOME_SNAPSHOT_VERSION,
        page,
    })
}

fn decode_youtube_home_snapshot(raw: &str) -> Option<YouTubeHomePage> {
    let snapshot = serde_json::from_str::<PersistedYouTubeHomeSnapshot>(raw).ok()?;
    if snapshot.version != HOME_SNAPSHOT_VERSION {
        return None;
    }

    let mut page = snapshot.page;
    page.sections.retain(|section| !section.items.is_empty());
    if page.sections.is_empty() {
        return None;
    }
    page.stale = true;
    Some(page)
}

#[cfg(test)]
mod tests {
    use super::{
        decode_youtube_home_snapshot, snapshot_from_page, PersistedYouTubeHomeSnapshot,
        HOME_SNAPSHOT_VERSION,
    };
    use crate::youtube::{YouTubeHomeChip, YouTubeHomePage, YouTubeHomeSection, YouTubeItem};

    fn playlist(title: &str, browse_id: &str) -> YouTubeItem {
        YouTubeItem {
            result_type: "playlist".to_string(),
            title: title.to_string(),
            browse_id: browse_id.to_string(),
            ..YouTubeItem::default()
        }
    }

    #[test]
    fn round_trip_preserves_home_order_and_navigation_metadata() {
        let page = YouTubeHomePage {
            version: 2,
            selected_chip_params: "energy".to_string(),
            chips: vec![YouTubeHomeChip {
                title: "Energy".to_string(),
                params: "energy".to_string(),
                ..YouTubeHomeChip::default()
            }],
            sections: vec![
                YouTubeHomeSection {
                    id: "first".to_string(),
                    title: "First".to_string(),
                    items: vec![playlist("One", "VL1")],
                    ..YouTubeHomeSection::default()
                },
                YouTubeHomeSection {
                    id: "second".to_string(),
                    title: "Second".to_string(),
                    items: vec![playlist("Two", "VL2")],
                    ..YouTubeHomeSection::default()
                },
            ],
            continuation: "next-page".to_string(),
            ..YouTubeHomePage::default()
        };

        let snapshot = snapshot_from_page(&page).expect("valid Home snapshot");
        let raw = serde_json::to_string(&snapshot).expect("serializable Home snapshot");
        let restored = decode_youtube_home_snapshot(&raw).expect("restorable Home snapshot");

        assert_eq!(restored.sections[0].items[0].browse_id, "VL1");
        assert_eq!(restored.sections[1].items[0].browse_id, "VL2");
        assert_eq!(restored.selected_chip_params, "energy");
        assert_eq!(restored.continuation, "next-page");
        assert!(restored.stale);
    }

    #[test]
    fn empty_home_never_replaces_a_valid_snapshot() {
        assert!(snapshot_from_page(&YouTubeHomePage::default()).is_none());
    }

    #[test]
    fn incompatible_snapshot_version_is_rejected() {
        let snapshot = PersistedYouTubeHomeSnapshot {
            version: HOME_SNAPSHOT_VERSION + 1,
            page: YouTubeHomePage {
                sections: vec![YouTubeHomeSection {
                    items: vec![playlist("One", "VL1")],
                    ..YouTubeHomeSection::default()
                }],
                ..YouTubeHomePage::default()
            },
        };
        let raw = serde_json::to_string(&snapshot).expect("serializable Home snapshot");

        assert!(decode_youtube_home_snapshot(&raw).is_none());
    }
}
