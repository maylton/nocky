use crate::{
    app::controller::AppController,
    youtube::{cached_cover_for_item, YouTubeHomePage},
};
use gtk::glib;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

const HOME_SNAPSHOT_VERSION: u32 = 1;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
struct PersistedYouTubeHomeSnapshot {
    version: u32,
    page: YouTubeHomePage,
}

pub(super) struct DurableHomeSnapshot {
    path: PathBuf,
    last_saved: Option<YouTubeHomePage>,
}

impl DurableHomeSnapshot {
    pub(super) fn load(controller: &AppController) -> Self {
        let path = home_snapshot_path();
        let restored = fs::read_to_string(&path)
            .ok()
            .and_then(|raw| decode_snapshot(&raw));

        if let Some(page) = restored.as_ref() {
            let mut current = controller.youtube_home_page.borrow_mut();
            if current.sections.is_empty() {
                *current = page.clone();
            }
        }

        Self {
            path,
            last_saved: restored,
        }
    }

    pub(super) fn clear(&mut self) {
        self.last_saved = None;

        let temporary = self.path.with_extension("json.tmp");
        for path in [&self.path, &temporary] {
            if let Err(error) = fs::remove_file(path) {
                if error.kind() != std::io::ErrorKind::NotFound {
                    eprintln!(
                        "Could not remove YouTube Home snapshot {}: {error}",
                        path.display()
                    );
                }
            }
        }
    }

    pub(super) fn persist_if_changed(&mut self, page: &YouTubeHomePage) {
        let Some(page) = valid_page(page) else {
            return;
        };
        if self.last_saved.as_ref() == Some(&page) {
            return;
        }

        let snapshot = PersistedYouTubeHomeSnapshot {
            version: HOME_SNAPSHOT_VERSION,
            page: page.clone(),
        };
        match save_snapshot(&self.path, &snapshot) {
            Ok(()) => self.last_saved = Some(page),
            Err(error) => eprintln!("Could not persist YouTube Home snapshot: {error}"),
        }
    }
}

fn home_snapshot_path() -> PathBuf {
    glib::user_cache_dir()
        .join("nocky")
        .join("youtube")
        .join("home-snapshot-v1.json")
}

fn valid_page(page: &YouTubeHomePage) -> Option<YouTubeHomePage> {
    let mut page = page.clone();
    page.sections.retain(|section| !section.items.is_empty());
    (!page.sections.is_empty()).then_some(page)
}

fn repair_home_snapshot_cover_paths(page: &mut YouTubeHomePage) -> bool {
    let mut changed = false;

    for section in &mut page.sections {
        for item in &mut section.items {
            let cover_missing_or_invalid =
                item.cover_path.trim().is_empty() || !Path::new(item.cover_path.trim()).is_file();
            if !cover_missing_or_invalid {
                continue;
            }

            if let Some(path) = cached_cover_for_item(item) {
                let repaired = path.to_string_lossy().into_owned();
                if item.cover_path != repaired {
                    item.cover_path = repaired;
                    changed = true;
                }
            }
        }
    }

    changed
}

fn decode_snapshot(raw: &str) -> Option<YouTubeHomePage> {
    let snapshot = serde_json::from_str::<PersistedYouTubeHomeSnapshot>(raw).ok()?;
    if snapshot.version != HOME_SNAPSHOT_VERSION {
        return None;
    }

    let mut page = valid_page(&snapshot.page)?;
    repair_home_snapshot_cover_paths(&mut page);
    page.stale = true;
    Some(page)
}

fn save_snapshot(
    path: &std::path::Path,
    snapshot: &PersistedYouTubeHomeSnapshot,
) -> Result<(), String> {
    let Some(parent) = path.parent() else {
        return Err("YouTube Home snapshot path has no parent directory".to_string());
    };
    fs::create_dir_all(parent)
        .map_err(|error| format!("Could not create YouTube Home snapshot folder: {error}"))?;

    let raw = serde_json::to_vec(snapshot)
        .map_err(|error| format!("Could not serialize YouTube Home snapshot: {error}"))?;
    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, raw)
        .map_err(|error| format!("Could not write YouTube Home snapshot: {error}"))?;
    fs::rename(&temporary, path)
        .map_err(|error| format!("Could not replace YouTube Home snapshot: {error}"))
}

#[cfg(test)]
mod tests {
    use super::{decode_snapshot, valid_page, PersistedYouTubeHomeSnapshot, HOME_SNAPSHOT_VERSION};
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
        let snapshot = PersistedYouTubeHomeSnapshot {
            version: HOME_SNAPSHOT_VERSION,
            page,
        };
        let raw = serde_json::to_string(&snapshot).expect("serializable Home snapshot");
        let restored = decode_snapshot(&raw).expect("restorable Home snapshot");

        assert_eq!(restored.sections[0].items[0].browse_id, "VL1");
        assert_eq!(restored.sections[1].items[0].browse_id, "VL2");
        assert_eq!(restored.selected_chip_params, "energy");
        assert_eq!(restored.continuation, "next-page");
        assert!(restored.stale);
    }

    #[test]
    fn persisted_snapshot_contains_appended_home_continuation() {
        let mut page = YouTubeHomePage {
            sections: vec![YouTubeHomeSection {
                id: "first".to_string(),
                title: "First".to_string(),
                items: vec![playlist("One", "VL1")],
                ..YouTubeHomeSection::default()
            }],
            continuation: "page-2".to_string(),
            ..YouTubeHomePage::default()
        };
        page.append_continuation(YouTubeHomePage {
            sections: vec![YouTubeHomeSection {
                id: "second".to_string(),
                title: "Second".to_string(),
                items: vec![playlist("Two", "VL2")],
                ..YouTubeHomeSection::default()
            }],
            continuation: "page-3".to_string(),
            ..YouTubeHomePage::default()
        });

        let snapshot = PersistedYouTubeHomeSnapshot {
            version: HOME_SNAPSHOT_VERSION,
            page,
        };
        let raw = serde_json::to_string(&snapshot).expect("serializable Home snapshot");
        let restored = decode_snapshot(&raw).expect("restorable Home snapshot");

        assert_eq!(
            restored
                .sections
                .iter()
                .map(|section| section.id.as_str())
                .collect::<Vec<_>>(),
            ["first", "second"]
        );
        assert_eq!(restored.continuation, "page-3");
    }

    #[test]
    fn empty_home_is_not_persisted() {
        assert!(valid_page(&YouTubeHomePage::default()).is_none());
    }

    #[test]
    fn restored_snapshot_preserves_thumbnail_when_cover_path_is_missing() {
        let mut item = playlist("One", "VL1");
        item.thumbnail_url = "https://example.invalid/cover.jpg".to_string();
        item.cover_path = String::new();
        let page = YouTubeHomePage {
            sections: vec![YouTubeHomeSection {
                id: "first".to_string(),
                title: "First".to_string(),
                items: vec![item],
                ..YouTubeHomeSection::default()
            }],
            ..YouTubeHomePage::default()
        };
        let snapshot = PersistedYouTubeHomeSnapshot {
            version: HOME_SNAPSHOT_VERSION,
            page,
        };
        let raw = serde_json::to_string(&snapshot).expect("serializable Home snapshot");
        let restored = decode_snapshot(&raw).expect("restorable Home snapshot");

        let restored_item = &restored.sections[0].items[0];
        assert_eq!(
            restored_item.thumbnail_url,
            "https://example.invalid/cover.jpg"
        );
        assert!(restored_item.cover_path.is_empty());
        assert!(restored.stale);
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

        assert!(decode_snapshot(&raw).is_none());
    }
}
