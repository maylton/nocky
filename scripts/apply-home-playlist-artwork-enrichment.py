#!/usr/bin/env python3
"""Apply playlist artwork enrichment and final chip spacing."""

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
    scroll.set_min_content_height(76);
    scroll.set_propagate_natural_height(true);
''',
    "chip carousel final height",
)

replace_once(
    "helpers/nocky_youtube.py",
    '''def _cached_root_home_chips(section_limit: int) -> list[dict[str, str]]:
''',
    '''_HOME_ITEM_RENDERERS = {
    "musicTwoRowItemRenderer",
    "musicResponsiveListItemRenderer",
    "musicMultiRowListItemRenderer",
}


def _normalized_playlist_id(value: Any) -> str:
    playlist_id = _text(value)
    if playlist_id.startswith("VL"):
        playlist_id = playlist_id[2:]
    return playlist_id if playlist_id.startswith(("PL", "RD", "OLAK5uy_")) else ""


def _nested_values(node: Any, keys: set[str]) -> list[str]:
    values: list[str] = []
    if isinstance(node, dict):
        for key, value in node.items():
            if key in keys:
                text = _text(value)
                if text:
                    values.append(text)
            if isinstance(value, (dict, list, tuple)):
                values.extend(_nested_values(value, keys))
    elif isinstance(node, (list, tuple)):
        for value in node:
            values.extend(_nested_values(value, keys))
    return values


def _raw_home_playlist_artwork(response: Any) -> dict[str, tuple[str, str]]:
    artwork: dict[str, tuple[str, str]] = {}

    def walk(node: Any) -> None:
        if isinstance(node, dict):
            for key, value in node.items():
                if key in _HOME_ITEM_RENDERERS and isinstance(value, dict):
                    thumbnail = _best_thumbnail(value)
                    video_id = next(
                        (
                            candidate
                            for candidate in _nested_values(value, {"videoId"})
                            if VIDEO_ID_PATTERN.fullmatch(candidate)
                        ),
                        "",
                    )
                    for candidate in _nested_values(value, {"playlistId", "browseId"}):
                        playlist_id = _normalized_playlist_id(candidate)
                        if playlist_id and (thumbnail or video_id):
                            previous = artwork.get(playlist_id, ("", ""))
                            artwork[playlist_id] = (
                                thumbnail or previous[0],
                                video_id or previous[1],
                            )
                if isinstance(value, (dict, list, tuple)):
                    walk(value)
        elif isinstance(node, (list, tuple)):
            for value in node:
                walk(value)

    walk(response)
    return artwork


def _apply_raw_home_playlist_artwork(page: dict[str, Any], response: Any) -> None:
    artwork = _raw_home_playlist_artwork(response)
    if not artwork:
        return
    for section in page.get("sections") or []:
        for item in section.get("items") or []:
            if not isinstance(item, dict) or item.get("result_type") != "playlist":
                continue
            playlist_id = _normalized_playlist_id(item.get("browse_id"))
            thumbnail, video_id = artwork.get(playlist_id, ("", ""))
            if not item.get("thumbnail_url") and thumbnail:
                item["thumbnail_url"] = thumbnail
            if not item.get("video_id") and video_id:
                item["video_id"] = video_id
            if not item.get("thumbnail_url") and video_id:
                item["thumbnail_url"] = f"https://i.ytimg.com/vi/{video_id}/hqdefault.jpg"


def _remote_playlist_artwork(client: Any, item: dict[str, Any]) -> tuple[str, str]:
    playlist_id = _normalized_playlist_id(item.get("browse_id"))
    video_id = _text(item.get("video_id"))
    if not playlist_id:
        return "", video_id

    try:
        data = client.get_playlist(playlist_id, limit=1)
        if isinstance(data, dict):
            thumbnail = _best_thumbnail(data)
            tracks = data.get("tracks") or []
            first_track = next((track for track in tracks if isinstance(track, dict)), {})
            thumbnail = thumbnail or _best_thumbnail(first_track)
            first_video = _text(first_track.get("videoId") or first_track.get("video_id"))
            if thumbnail or first_video:
                return thumbnail, video_id or first_video
    except Exception:
        pass

    try:
        watch = client.get_watch_playlist(
            videoId=video_id or None,
            playlistId=playlist_id,
            limit=1,
            radio=playlist_id.startswith("RD"),
        )
        if isinstance(watch, dict):
            tracks = watch.get("tracks") or []
            first_track = next((track for track in tracks if isinstance(track, dict)), {})
            thumbnail = _best_thumbnail(first_track)
            first_video = _text(first_track.get("videoId") or first_track.get("video_id"))
            return thumbnail, video_id or first_video
    except Exception:
        pass
    return "", video_id


def _enrich_missing_playlist_artwork(
    client: Any,
    page: dict[str, Any],
    *,
    limit: int = 24,
) -> None:
    pending: list[dict[str, Any]] = []
    for section in page.get("sections") or []:
        for item in section.get("items") or []:
            if (
                isinstance(item, dict)
                and item.get("result_type") == "playlist"
                and not _text(item.get("thumbnail_url"))
                and _normalized_playlist_id(item.get("browse_id"))
            ):
                pending.append(item)

    unresolved: list[str] = []
    for item in pending[: max(0, limit)]:
        thumbnail, video_id = _remote_playlist_artwork(client, item)
        if video_id and not item.get("video_id"):
            item["video_id"] = video_id
        if thumbnail:
            item["thumbnail_url"] = _upgrade_thumbnail_url(thumbnail)
        elif video_id and VIDEO_ID_PATTERN.fullmatch(video_id):
            item["thumbnail_url"] = f"https://i.ytimg.com/vi/{video_id}/hqdefault.jpg"
        else:
            unresolved.append(_text(item.get("title")) or _text(item.get("browse_id")))

    if unresolved:
        preview = ", ".join(unresolved[:5])
        print(
            f"Nocky YouTube Home artwork unresolved for {len(unresolved)} playlist(s): {preview}",
            file=sys.stderr,
        )


def _cached_root_home_chips(section_limit: int) -> list[dict[str, str]]:
''',
    "Home playlist artwork helpers",
)

replace_once(
    "helpers/nocky_youtube.py",
    '''    client = _create_client(authenticated=True)

    try:
        fetch_limit = max(12, min(36, offset + section_limit + 1))
''',
    '''    client = _create_client(authenticated=True)

    try:
        raw_response: dict[str, Any] = {}
        fetch_limit = max(12, min(36, offset + section_limit + 1))
''',
    "initialize raw Home response",
)
replace_once(
    "helpers/nocky_youtube.py",
    '''        if chips:
            page["chips"] = chips
        save_cached_page(_home_feed_cache_path(), cache_key, page)
''',
    '''        if chips:
            page["chips"] = chips
        _apply_raw_home_playlist_artwork(page, raw_response)
        _enrich_missing_playlist_artwork(client, page)
        save_cached_page(_home_feed_cache_path(), cache_key, page)
''',
    "enrich Home playlist artwork",
)

Path("tests/test_youtube_home_playlist_artwork.py").write_text(
    '''from __future__ import annotations

import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "helpers"))

import nocky_youtube  # noqa: E402


class FakeClient:
    def __init__(self, playlist=None, watch=None, fail_playlist=False):
        self.playlist = playlist or {}
        self.watch = watch or {}
        self.fail_playlist = fail_playlist

    def get_playlist(self, playlist_id, limit=1):
        if self.fail_playlist:
            raise RuntimeError("playlist unavailable")
        return self.playlist

    def get_watch_playlist(self, **kwargs):
        return self.watch


def page_item(playlist_id="PL-test"):
    return {
        "sections": [
            {
                "items": [
                    {
                        "result_type": "playlist",
                        "title": "Test playlist",
                        "browse_id": playlist_id,
                        "video_id": "",
                        "thumbnail_url": "",
                    }
                ]
            }
        ]
    }


class HomePlaylistArtworkTests(unittest.TestCase):
    def test_remote_header_artwork_fills_missing_playlist_cover(self):
        page = page_item()
        client = FakeClient(
            playlist={
                "thumbnails": [
                    {"url": "https://lh3.googleusercontent.com/header=s240", "width": 240, "height": 240}
                ],
                "tracks": [],
            }
        )
        nocky_youtube._enrich_missing_playlist_artwork(client, page)
        self.assertIn("header=s1200", page["sections"][0]["items"][0]["thumbnail_url"])

    def test_first_track_artwork_and_video_are_used_when_header_is_missing(self):
        page = page_item()
        client = FakeClient(
            playlist={
                "tracks": [
                    {
                        "videoId": "abcdefghijk",
                        "thumbnails": [
                            {"url": "https://lh3.googleusercontent.com/track=s240", "width": 240, "height": 240}
                        ],
                    }
                ]
            }
        )
        nocky_youtube._enrich_missing_playlist_artwork(client, page)
        item = page["sections"][0]["items"][0]
        self.assertEqual(item["video_id"], "abcdefghijk")
        self.assertIn("track=s1200", item["thumbnail_url"])

    def test_watch_playlist_fallback_handles_generated_playlist(self):
        page = page_item("RDTMAK5uy-test")
        client = FakeClient(
            fail_playlist=True,
            watch={
                "tracks": [
                    {
                        "videoId": "abcdefghijk",
                        "thumbnail": [
                            {"url": "https://lh3.googleusercontent.com/watch=s240", "width": 240, "height": 240}
                        ],
                    }
                ]
            },
        )
        nocky_youtube._enrich_missing_playlist_artwork(client, page)
        item = page["sections"][0]["items"][0]
        self.assertEqual(item["video_id"], "abcdefghijk")
        self.assertIn("watch=s1200", item["thumbnail_url"])

    def test_raw_renderer_artwork_is_applied_before_remote_lookup(self):
        page = page_item()
        raw = {
            "musicTwoRowItemRenderer": {
                "title": {
                    "runs": [
                        {
                            "text": "Test playlist",
                            "navigationEndpoint": {
                                "browseEndpoint": {"browseId": "VLPL-test"}
                            },
                        }
                    ]
                },
                "thumbnailRenderer": {
                    "musicThumbnailRenderer": {
                        "thumbnail": {
                            "thumbnails": [
                                {"url": "https://lh3.googleusercontent.com/raw=s240", "width": 240, "height": 240}
                            ]
                        }
                    }
                },
            }
        }
        nocky_youtube._apply_raw_home_playlist_artwork(page, raw)
        self.assertIn("raw=s1200", page["sections"][0]["items"][0]["thumbnail_url"])


if __name__ == "__main__":
    unittest.main()
''',
    encoding="utf-8",
)

print("Playlist artwork enrichment and chip spacing applied")
