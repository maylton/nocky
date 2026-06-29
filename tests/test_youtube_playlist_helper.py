from __future__ import annotations

import sys
import unittest
from pathlib import Path
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "helpers"))

import nocky_youtube_playlist  # noqa: E402


class FakeClient:
    def __init__(self, response):
        self.response = response
        self.calls = []

    def get_playlist(self, playlist_id, limit=100):
        self.calls.append((playlist_id, limit))
        return self.response


class YouTubePlaylistHelperTests(unittest.TestCase):
    def test_normalizes_vl_prefixed_playlist_id(self) -> None:
        self.assertEqual(
            nocky_youtube_playlist.normalize_playlist_id("VLPL-example_123"),
            "PL-example_123",
        )

    def test_rejects_urls_and_invalid_characters(self) -> None:
        for value in (
            "https://music.youtube.com/playlist?list=PL-example",
            "PL example",
            "<playlist>",
            "",
        ):
            with self.subTest(value=value):
                with self.assertRaisesRegex(RuntimeError, "Invalid YouTube Music"):
                    nocky_youtube_playlist.normalize_playlist_id(value)

    def test_requires_connected_session_before_client_creation(self) -> None:
        with patch.object(
            nocky_youtube_playlist.nocky_youtube,
            "_load_session",
            return_value={},
        ):
            with patch.object(
                nocky_youtube_playlist.nocky_youtube,
                "_create_client",
            ) as create_client:
                with self.assertRaisesRegex(RuntimeError, "Connect a YouTube Music"):
                    nocky_youtube_playlist.fetch_playlist_metadata("PL-example")
                create_client.assert_not_called()

    def test_invalid_id_does_not_reach_remote_client(self) -> None:
        with patch.object(
            nocky_youtube_playlist.nocky_youtube,
            "_load_session",
            return_value={"headers": {"test_header": "value"}},
        ):
            with patch.object(
                nocky_youtube_playlist.nocky_youtube,
                "_create_client",
            ) as create_client:
                with self.assertRaisesRegex(RuntimeError, "Invalid YouTube Music"):
                    nocky_youtube_playlist.fetch_playlist_metadata("not a playlist")
                create_client.assert_not_called()

    def test_fetches_and_sanitizes_playlist_detail(self) -> None:
        client = FakeClient(
            {
                "id": "PL-owned",
                "title": "Road Trip",
                "owned": True,
                "privacy": "PRIVATE",
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

        with patch.object(
            nocky_youtube_playlist.nocky_youtube,
            "_load_session",
            return_value={"headers": {"test_header": "value"}},
        ):
            with patch.object(
                nocky_youtube_playlist.nocky_youtube,
                "_create_client",
                return_value=client,
            ):
                result = nocky_youtube_playlist.fetch_playlist_metadata(
                    "VLPL-owned",
                    900,
                )

        self.assertEqual(client.calls, [("PL-owned", 500)])
        self.assertTrue(result["editable"])
        self.assertEqual(result["tracks"][0]["set_video_id"], "set-video-1")
        self.assertNotIn("private_field", str(result))

    def test_rejects_invalid_remote_response(self) -> None:
        client = FakeClient([])
        with patch.object(
            nocky_youtube_playlist.nocky_youtube,
            "_load_session",
            return_value={"headers": {"test_header": "value"}},
        ):
            with patch.object(
                nocky_youtube_playlist.nocky_youtube,
                "_create_client",
                return_value=client,
            ):
                with self.assertRaisesRegex(RuntimeError, "invalid playlist response"):
                    nocky_youtube_playlist.fetch_playlist_metadata("PL-example")


if __name__ == "__main__":
    unittest.main()
