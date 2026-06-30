#!/usr/bin/env python3
"""Apply the final Home V2 artwork priority and chip-height adjustments."""

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
    rail.set_margin_bottom(10);
''',
    '''    rail.set_margin_start(2);
    rail.set_margin_end(28);
    rail.set_margin_bottom(18);
''',
    "Home V2 chip rail margin",
)
replace_once(
    "src/browser.rs",
    '''    scroll.set_hexpand(true);
    scroll.set_min_content_height(52);
    scroll.set_propagate_natural_height(true);
''',
    '''    scroll.set_hexpand(true);
    scroll.set_min_content_height(64);
    scroll.set_propagate_natural_height(true);
''',
    "Home V2 chip scroll height",
)

replace_once(
    "src/youtube/mod.rs",
    '''pub fn cache_home_page_covers(page: &mut YouTubeHomePage) {
    const PAGE_COVER_BUDGET: usize = 24;
    let mut remaining = PAGE_COVER_BUDGET;

    for section in &mut page.sections {
        if remaining == 0 || !structured_cards::uses_card_carousel(&section.layout) {
            continue;
        }

        let count = section.items.len().min(remaining);
        cache_items_for_browser(&mut section.items[..count]);
        remaining -= count;
    }
}
''',
    '''pub fn cache_home_page_covers(page: &mut YouTubeHomePage) {
    const PAGE_COVER_BUDGET: usize = 64;
    let mut remaining = PAGE_COVER_BUDGET;

    // Collection cards are visually dominated by their artwork and often appear
    // after song-heavy rows. Cache them first so a fixed network budget does not
    // leave playlists, albums and artists as placeholders farther down the feed.
    for collection_pass in [true, false] {
        for section in &mut page.sections {
            if !structured_cards::uses_card_carousel(&section.layout) {
                continue;
            }

            for item in &mut section.items {
                if remaining == 0 {
                    return;
                }
                let collection = matches!(
                    item.result_type.as_str(),
                    "playlist" | "album" | "artist" | "podcast"
                );
                if collection != collection_pass || item.thumbnail_url.trim().is_empty() {
                    continue;
                }
                cache_items_for_browser(std::slice::from_mut(item));
                remaining -= 1;
            }
        }
    }
}
''',
    "Home V2 cover caching priority",
)

replace_once(
    "helpers/nocky_youtube_feed.py",
    '''CONTRACT_VERSION = 2
DEFAULT_CACHE_MAX_AGE = 12 * 60 * 60
ItemFactory = Callable[[dict[str, Any], str], dict[str, Any] | None]
''',
    '''CONTRACT_VERSION = 2
DEFAULT_CACHE_MAX_AGE = 12 * 60 * 60
VIDEO_ID_PATTERN = re.compile(r"^[A-Za-z0-9_-]{11}$")
ItemFactory = Callable[[dict[str, Any], str], dict[str, Any] | None]
''',
    "feed video ID pattern",
)
replace_once(
    "helpers/nocky_youtube_feed.py",
    '''def _best_thumbnail(value: Any) -> str:
    candidates = _thumbnail_candidates(value)
    if not candidates:
        return ""
    candidate = max(candidates, key=_thumbnail_area)
    return _upgrade_thumbnail_url(_text(candidate.get("url")))


def _duration_seconds(result: dict[str, Any]) -> int:
''',
    '''def _best_thumbnail(value: Any) -> str:
    candidates = _thumbnail_candidates(value)
    if not candidates:
        return ""
    candidate = max(candidates, key=_thumbnail_area)
    return _upgrade_thumbnail_url(_text(candidate.get("url")))


def _video_thumbnail(video_id: str) -> str:
    video_id = _text(video_id)
    if not VIDEO_ID_PATTERN.fullmatch(video_id):
        return ""
    return f"https://i.ytimg.com/vi/{video_id}/hqdefault.jpg"


def _duration_seconds(result: dict[str, Any]) -> int:
''',
    "feed video thumbnail fallback helper",
)
replace_once(
    "helpers/nocky_youtube_feed.py",
    '''        "duration_seconds": duration,
        "thumbnail_url": _best_thumbnail(result),
    }
''',
    '''        "duration_seconds": duration,
        "thumbnail_url": _best_thumbnail(result) or _video_thumbnail(video_id),
    }
''',
    "feed item thumbnail fallback",
)

replace_once(
    "tests/test_youtube_feed.py",
    '''    def test_deduplicates_items_without_flattening_sections(self) -> None:
''',
    '''    def test_uses_video_thumbnail_when_playlist_artwork_is_missing(self) -> None:
        source = {
            "sections": [
                {
                    "title": "Playlists",
                    "contents": [
                        {
                            "resultType": "playlist",
                            "title": "Fallback playlist",
                            "playlistId": "PL-fallback",
                            "videoId": "abcdefghijk",
                        }
                    ],
                }
            ]
        }
        page = build_structured_home(source, section_limit=1)
        item = page["sections"][0]["items"][0]
        self.assertEqual(
            item["thumbnail_url"],
            "https://i.ytimg.com/vi/abcdefghijk/hqdefault.jpg",
        )

    def test_deduplicates_items_without_flattening_sections(self) -> None:
''',
    "playlist artwork fallback test",
)

print("Final Home V2 artwork and chip fixes applied")
