from __future__ import annotations

import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "helpers"))

from nocky_playlist_metadata import normalize_playlist_detail  # noqa: E402


class YouTubePlaylistMetadataTests(unittest.TestCase):
    def test_preserves_owned_privacy_and_track_occurrence_identity(self) -> None:
        result = normalize_playlist_detail(
            {
                "id": "PL-owned",
                "title": "Road Trip",
                "owned": True,
                "privacy": "PRIVATE",
                "tracks": [
                    {
                        "videoId": "video-1",
                        "setVideoId": "set-video-1",
                        "title": "Song",
                    },
                    {
                        "videoId": "video-1",
                        "setVideoId": "set-video-2",
                        "title": "Song",
                    },
                ],
            }
        )

        self.assertTrue(result["owned"])
        self.assertTrue(result["editable"])
        self.assertEqual(result["privacy"], "PRIVATE")
        self.assertEqual(len(result["tracks"]), 2)
        self.assertEqual(result["tracks"][0]["set_video_id"], "set-video-1")
        self.assertEqual(result["tracks"][1]["set_video_id"], "set-video-2")

    def test_non_owned_playlist_is_not_editable(self) -> None:
        result = normalize_playlist_detail(
            {
                "playlistId": "PL-shared",
                "title": "Shared",
                "owned": False,
                "privacyStatus": "UNLISTED",
            }
        )

        self.assertFalse(result["owned"])
        self.assertFalse(result["editable"])
        self.assertEqual(result["privacy"], "UNLISTED")

    def test_strips_vl_prefix_from_browse_identity(self) -> None:
        result = normalize_playlist_detail(
            {
                "browseId": "VLPL-prefixed",
                "title": "Prefixed",
                "owned": True,
            }
        )

        self.assertEqual(result["playlist_id"], "PL-prefixed")
        self.assertTrue(result["editable"])

    def test_unknown_privacy_is_not_invented(self) -> None:
        result = normalize_playlist_detail(
            {
                "id": "PL-owned",
                "title": "Playlist",
                "owned": True,
                "privacy": "FRIENDS_ONLY",
            }
        )

        self.assertEqual(result["privacy"], "")

    def test_incomplete_tracks_remain_read_only(self) -> None:
        result = normalize_playlist_detail(
            {
                "id": "PL-owned",
                "title": "Playlist",
                "owned": True,
                "tracks": [
                    {"videoId": "video-1", "title": "Song"},
                    {"setVideoId": "orphan-set-id", "title": "Ignored"},
                ],
            }
        )

        self.assertEqual(
            result["tracks"],
            [
                {
                    "video_id": "video-1",
                    "set_video_id": "",
                    "title": "Song",
                }
            ],
        )

    def test_contract_excludes_unrelated_payload_fields(self) -> None:
        result = normalize_playlist_detail(
            {
                "id": "PL-owned",
                "title": "Playlist",
                "owned": True,
                "private_field": "ignored",
                "tracks": [
                    {
                        "videoId": "video-1",
                        "setVideoId": "set-video-1",
                        "title": "Song",
                        "private_field": "ignored",
                    }
                ],
            }
        )

        self.assertEqual(
            set(result),
            {"playlist_id", "title", "owned", "privacy", "editable", "tracks"},
        )
        self.assertEqual(
            set(result["tracks"][0]),
            {"video_id", "set_video_id", "title"},
        )
        self.assertNotIn("private_field", str(result))

    def test_invalid_payload_degrades_to_empty_contract(self) -> None:
        self.assertEqual(
            normalize_playlist_detail(None),
            {
                "playlist_id": "",
                "title": "",
                "owned": False,
                "privacy": "",
                "editable": False,
                "tracks": [],
            },
        )


if __name__ == "__main__":
    unittest.main()
