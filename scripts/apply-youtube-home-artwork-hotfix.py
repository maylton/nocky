#!/usr/bin/env python3
from __future__ import annotations

from pathlib import Path

ROOT = Path.cwd()
BACKGROUND = ROOT / "src/app/controller/background.rs"
FEED = ROOT / "src/youtube/feed.rs"
SNAPSHOT = ROOT / "src/app/controller/callbacks/playlist_cache_first/home_snapshot.rs"


class PatchError(RuntimeError):
    pass


def replace_once(path: Path, old: str, new: str, label: str) -> None:
    text = path.read_text(encoding="utf-8")
    if new in text and old not in text:
        print(f"[already applied] {label}")
        return
    count = text.count(old)
    if count != 1:
        raise PatchError(f"{label}: expected one match in {path}, found {count}")
    path.write_text(text.replace(old, new, 1), encoding="utf-8")
    print(f"[changed] {label}")


def patch_background() -> None:
    old = '''                        let mut current = self.youtube_home_page.borrow_mut();
                        let delta = current.update_cover_paths_delta(&page);
                        let current_page = current.clone();
                        drop(current);
                        if !delta.sections.is_empty() && youtube_active {
                            if append {
                                let playback = self.browser_playback_state();
                                let appended = self.browser.append_youtube_home_page(
                                    &current_page,
                                    &delta,
                                    &playback,
                                    &self.config.borrow(),
                                );
                                if !appended {
                                    self.refresh_browser();
                                }
                            } else {
                                self.refresh_browser();
                            }
                        }
'''
    new = '''                        let mut current = self.youtube_home_page.borrow_mut();
                        let delta = current.update_cover_paths_delta(&page);
                        drop(current);
                        if !delta.sections.is_empty() && youtube_active {
                            // Cover-cache deltas mutate artwork on existing Home items. They are
                            // not continuation deltas, so rebuild the visible Home instead of
                            // routing them through append_youtube_home_page.
                            self.refresh_browser();
                        }
'''
    replace_once(BACKGROUND, old, new, "Treat Home artwork deltas as repaint, not continuation append")


def patch_snapshot() -> None:
    replace_once(
        SNAPSHOT,
        'use crate::{app::controller::AppController, youtube::YouTubeHomePage};\n',
        'use crate::{\n    app::controller::AppController,\n    youtube::{cached_cover_for_item, YouTubeHomePage},\n};\n',
        "Import cached cover repair helper",
    )
    replace_once(
        SNAPSHOT,
        'use std::{fs, path::PathBuf};\n',
        'use std::{\n    fs,\n    path::{Path, PathBuf},\n};\n',
        "Import Path for cover validation",
    )
    replace_once(
        SNAPSHOT,
        '''fn valid_page(page: &YouTubeHomePage) -> Option<YouTubeHomePage> {
    let mut page = page.clone();
    page.sections.retain(|section| !section.items.is_empty());
    (!page.sections.is_empty()).then_some(page)
}

fn decode_snapshot(raw: &str) -> Option<YouTubeHomePage> {
''',
        '''fn valid_page(page: &YouTubeHomePage) -> Option<YouTubeHomePage> {
    let mut page = page.clone();
    page.sections.retain(|section| !section.items.is_empty());
    (!page.sections.is_empty()).then_some(page)
}

fn repair_home_snapshot_cover_paths(page: &mut YouTubeHomePage) -> bool {
    let mut changed = false;

    for section in &mut page.sections {
        for item in &mut section.items {
            let cover_missing_or_invalid = item.cover_path.trim().is_empty()
                || !Path::new(item.cover_path.trim()).is_file();
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
''',
        "Add snapshot cover-path repair",
    )
    replace_once(
        SNAPSHOT,
        '''    let mut page = valid_page(&snapshot.page)?;
    page.stale = true;
    Some(page)
}
''',
        '''    let mut page = valid_page(&snapshot.page)?;
    repair_home_snapshot_cover_paths(&mut page);
    page.stale = true;
    Some(page)
}
''',
        "Repair restored Home snapshot covers before rendering",
    )
    replace_once(
        SNAPSHOT,
        '''    #[test]
    fn incompatible_snapshot_version_is_rejected() {
''',
        '''    #[test]
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
        assert_eq!(restored_item.thumbnail_url, "https://example.invalid/cover.jpg");
        assert!(restored_item.cover_path.is_empty());
        assert!(restored.stale);
    }

    #[test]
    fn incompatible_snapshot_version_is_rejected() {
''',
        "Add snapshot thumbnail preservation test",
    )


def patch_feed_tests() -> None:
    marker = "cover_delta_reports_changed_cover_paths"
    text = FEED.read_text(encoding="utf-8")
    if marker in text:
        print("[already applied] Add Home artwork delta tests")
        return
    insertion = r'''

    #[test]
    fn cover_delta_reports_changed_cover_paths() {
        let mut current_item = item("one", "One");
        current_item.thumbnail_url = "https://example.invalid/old.jpg".to_string();
        current_item.cover_path = "/tmp/old.cover".to_string();
        let mut incoming_item = item("one", "One");
        incoming_item.thumbnail_url = "https://example.invalid/new.jpg".to_string();
        incoming_item.cover_path = "/tmp/new.cover".to_string();
        let mut current = YouTubeHomePage {
            sections: vec![section("first", "carousel", vec![current_item])],
            ..YouTubeHomePage::default()
        };
        let incoming = YouTubeHomePage {
            sections: vec![section("first", "carousel", vec![incoming_item])],
            ..YouTubeHomePage::default()
        };

        let delta = current.update_cover_paths_delta(&incoming);

        assert_eq!(delta.sections.len(), 1);
        assert_eq!(current.sections[0].items[0].thumbnail_url, "https://example.invalid/new.jpg");
        assert_eq!(current.sections[0].items[0].cover_path, "/tmp/new.cover");
    }

    #[test]
    fn cover_delta_ignores_empty_incoming_artwork() {
        let mut current_item = item("one", "One");
        current_item.thumbnail_url = "https://example.invalid/old.jpg".to_string();
        current_item.cover_path = "/tmp/old.cover".to_string();
        let incoming_item = item("one", "One");
        let mut current = YouTubeHomePage {
            sections: vec![section("first", "carousel", vec![current_item])],
            ..YouTubeHomePage::default()
        };
        let incoming = YouTubeHomePage {
            sections: vec![section("first", "carousel", vec![incoming_item])],
            ..YouTubeHomePage::default()
        };

        let delta = current.update_cover_paths_delta(&incoming);

        assert!(delta.sections.is_empty());
        assert_eq!(current.sections[0].items[0].thumbnail_url, "https://example.invalid/old.jpg");
        assert_eq!(current.sections[0].items[0].cover_path, "/tmp/old.cover");
    }

    #[test]
    fn cover_delta_updates_thumbnail_without_clearing_existing_cover() {
        let mut current_item = item("one", "One");
        current_item.thumbnail_url = "https://example.invalid/old.jpg".to_string();
        current_item.cover_path = "/tmp/old.cover".to_string();
        let mut incoming_item = item("one", "One");
        incoming_item.thumbnail_url = "https://example.invalid/new.jpg".to_string();
        let mut current = YouTubeHomePage {
            sections: vec![section("first", "carousel", vec![current_item])],
            ..YouTubeHomePage::default()
        };
        let incoming = YouTubeHomePage {
            sections: vec![section("first", "carousel", vec![incoming_item])],
            ..YouTubeHomePage::default()
        };

        let delta = current.update_cover_paths_delta(&incoming);

        assert_eq!(delta.sections.len(), 1);
        assert_eq!(current.sections[0].items[0].thumbnail_url, "https://example.invalid/new.jpg");
        assert_eq!(current.sections[0].items[0].cover_path, "/tmp/old.cover");
    }
'''
    index = text.rfind("\n}")
    if index == -1:
        raise PatchError("Could not find final test module brace in src/youtube/feed.rs")
    FEED.write_text(text[:index] + insertion + text[index:], encoding="utf-8")
    print("[changed] Add Home artwork delta tests")


def main() -> None:
    for path in [BACKGROUND, FEED, SNAPSHOT]:
        if not path.is_file():
            raise PatchError(f"Missing expected file: {path}")
    patch_background()
    patch_snapshot()
    patch_feed_tests()
    print("YouTube Home artwork hotfix applied successfully.")


if __name__ == "__main__":
    main()
