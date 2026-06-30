#!/usr/bin/env python3
"""Apply generated-playlist track artwork fallbacks and final chip spacing."""

from pathlib import Path


def replace_once(path: str, old: str, new: str, label: str) -> None:
    file = Path(path)
    text = file.read_text(encoding="utf-8")
    count = text.count(old)
    if count != 1:
        raise RuntimeError(f"{label}: expected one match in {path}, found {count}")
    file.write_text(text.replace(old, new, 1), encoding="utf-8")


replace_once(
    "src/browser.rs",
    '''    rail.set_margin_start(2);
    rail.set_margin_end(28);
    rail.set_margin_bottom(18);
''',
    '''    rail.set_margin_top(4);
    rail.set_margin_start(2);
    rail.set_margin_end(28);
    rail.set_margin_bottom(24);
''',
    "chip rail final inset",
)
replace_once(
    "src/browser.rs",
    '''    scroll.set_hexpand(true);
    scroll.set_min_content_height(64);
    scroll.set_propagate_natural_height(true);
''',
    '''    scroll.set_hexpand(true);
    scroll.set_min_content_height(80);
    scroll.set_propagate_natural_height(true);
''',
    "chip carousel final height",
)

replace_once(
    "helpers/nocky_youtube.py",
    '''    duration = _duration_seconds(result)
    subtitle = " • ".join(value for value in (artist, album, _format_duration(duration)) if value)
    return {
''',
    '''    duration = _duration_seconds(result)
    thumbnail_url = _best_thumbnail(_thumbnails(result))
    if not thumbnail_url and VIDEO_ID_PATTERN.fullmatch(video_id):
        thumbnail_url = f"https://i.ytimg.com/vi/{video_id}/hqdefault.jpg"
    subtitle = " • ".join(value for value in (artist, album, _format_duration(duration)) if value)
    return {
''',
    "track thumbnail fallback preparation",
)
replace_once(
    "helpers/nocky_youtube.py",
    '''        "duration_seconds": duration,
        "thumbnail_url": _best_thumbnail(_thumbnails(result)),
    }
''',
    '''        "duration_seconds": duration,
        "thumbnail_url": thumbnail_url,
    }
''',
    "track thumbnail fallback assignment",
)

replace_once(
    "src/youtube/mod.rs",
    '''pub fn cache_items_for_browser(items: &mut [YouTubeItem]) {
    for item in items {
        item.thumbnail_url = upgrade_thumbnail_url(&item.thumbnail_url, PLAYER_COVER_SIZE);
        if item.cover_path.is_empty() {
            if let Some(path) = download_cover_sized(item, &item.thumbnail_url, BROWSER_COVER_SIZE)
            {
                item.cover_path = path.to_string_lossy().to_string();
            }
        }
    }
}
''',
    '''fn youtube_video_thumbnail_url(video_id: &str) -> Option<String> {
    let video_id = video_id.trim();
    (video_id.len() == 11
        && video_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-')))
    .then(|| format!("https://i.ytimg.com/vi/{video_id}/hqdefault.jpg"))
}

pub fn cache_items_for_browser(items: &mut [YouTubeItem]) {
    for item in items {
        item.thumbnail_url = upgrade_thumbnail_url(&item.thumbnail_url, PLAYER_COVER_SIZE);
        let video_fallback = youtube_video_thumbnail_url(&item.video_id);
        if item.thumbnail_url.trim().is_empty() {
            if let Some(fallback) = video_fallback.as_ref() {
                item.thumbnail_url = fallback.clone();
            }
        }

        if item.cover_path.is_empty() {
            let primary_url = item.thumbnail_url.clone();
            let mut downloaded = download_cover_sized(item, &primary_url, BROWSER_COVER_SIZE);

            if downloaded.is_none() {
                if let Some(fallback) = video_fallback {
                    if fallback != primary_url {
                        item.thumbnail_url = fallback.clone();
                        downloaded = download_cover_sized(item, &fallback, BROWSER_COVER_SIZE);
                    }
                }
            }

            if let Some(path) = downloaded {
                item.cover_path = path.to_string_lossy().to_string();
            }
        }
    }
}
''',
    "track cover download fallback",
)

Path("tests/test_youtube_track_artwork.py").write_text(
    '''from __future__ import annotations

import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "helpers"))

import nocky_youtube  # noqa: E402


class YouTubeTrackArtworkTests(unittest.TestCase):
    def test_song_without_thumbnail_uses_video_thumbnail(self) -> None:
        item = nocky_youtube._song_item(
            {
                "videoId": "abcdefghijk",
                "title": "Track without artwork",
                "artists": [{"name": "Artist"}],
            }
        )
        self.assertIsNotNone(item)
        self.assertEqual(
            item["thumbnail_url"],
            "https://i.ytimg.com/vi/abcdefghijk/hqdefault.jpg",
        )

    def test_song_keeps_explicit_youtube_music_thumbnail(self) -> None:
        item = nocky_youtube._song_item(
            {
                "videoId": "abcdefghijk",
                "title": "Track with artwork",
                "thumbnails": [
                    {
                        "url": "https://lh3.googleusercontent.com/track=s240",
                        "width": 240,
                        "height": 240,
                    }
                ],
            }
        )
        self.assertIsNotNone(item)
        self.assertIn("track=s1200", item["thumbnail_url"])

    def test_invalid_video_id_does_not_create_synthetic_url(self) -> None:
        self.assertIsNone(
            nocky_youtube._song_item(
                {"videoId": "invalid", "title": "Invalid track"}
            )
        )


if __name__ == "__main__":
    unittest.main()
''',
    encoding="utf-8",
)

print("Generated-playlist track artwork and chip spacing applied")
