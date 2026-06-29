from __future__ import annotations

import sys
import unittest
from pathlib import Path
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "helpers"))

import nocky_playlist_mutations  # noqa: E402
import nocky_youtube_playlist_create  # noqa: E402


class FakeClient:
    def __init__(self, response=None):
        self.response = response or {
            "id": "PL-owned",
            "title": "Focus",
            "owned": True,
            "privacy": "PRIVATE",
            "tracks": [
                {
                    "videoId": "abcdefghijk",
                    "setVideoId": "set-occurrence-1",
                    "title": "Track",
                }
            ],
        }
        self.calls = []

    def get_playlist(self, playlist_id, limit=100):
        self.calls.append((playlist_id, limit))
        return self.response


class PackagedPlaylistMetadataTests(unittest.TestCase):
    def test_normalizer_preserves_only_allowlisted_editability_fields(self) -> None:
        result = nocky_playlist_mutations.normalize_playlist_detail(
            {
                "id": "PL-owned",
                "title": "Focus",
                "owned": True,
                "privacyStatus": "PRIVATE",
                "private_context": "ignored",
                "tracks": [
                    {
                        "videoId": "abcdefghijk",
                        "setVideoId": "set-occurrence-1",
                        "title": "Track",
                        "private_context": "ignored",
                    }
                ],
            }
        )

        self.assertEqual(
            set(result),
            {"playlist_id", "title", "owned", "privacy", "editable", "tracks"},
        )
        self.assertTrue(result["owned"])
        self.assertTrue(result["editable"])
        self.assertEqual(result["privacy"], "PRIVATE")
        self.assertEqual(
            set(result["tracks"][0]),
            {"video_id", "set_video_id", "title"},
        )
        self.assertNotIn("private_context", str(result))

    def test_metadata_request_validates_id_before_session_access(self) -> None:
        with patch.object(
            nocky_youtube_playlist_create.nocky_youtube,
            "_load_session",
        ) as load_session:
            with self.assertRaisesRegex(RuntimeError, "Invalid YouTube Music"):
                nocky_youtube_playlist_create.fetch_playlist_metadata(
                    {"playlist_id": "https://music.youtube.com/playlist"}
                )
            load_session.assert_not_called()

    def test_disconnected_metadata_request_does_not_create_client(self) -> None:
        with patch.object(
            nocky_youtube_playlist_create.nocky_youtube,
            "_load_session",
            return_value={},
        ):
            with patch.object(
                nocky_youtube_playlist_create.nocky_youtube,
                "_create_client",
            ) as create_client:
                with self.assertRaisesRegex(RuntimeError, "Connect a YouTube Music"):
                    nocky_youtube_playlist_create.fetch_playlist_metadata(
                        {"playlist_id": "PL-owned"}
                    )
                create_client.assert_not_called()

    def test_metadata_operation_uses_authenticated_client_and_safe_limit(self) -> None:
        client = FakeClient()
        with patch.object(
            nocky_youtube_playlist_create.nocky_youtube,
            "_load_session",
            return_value={"headers": {"test": "value"}},
        ):
            with patch.object(
                nocky_youtube_playlist_create.nocky_youtube,
                "_create_client",
                return_value=client,
            ) as create_client:
                result = nocky_youtube_playlist_create.execute(
                    {
                        "operation": "metadata",
                        "playlist_id": "VLPL-owned",
                        "limit": 900,
                    }
                )

        create_client.assert_called_once_with(authenticated=True)
        self.assertEqual(client.calls, [("PL-owned", 500)])
        self.assertEqual(result["playlist_id"], "PL-owned")
        self.assertEqual(result["tracks"][0]["set_video_id"], "set-occurrence-1")

    def test_mismatched_metadata_is_rejected(self) -> None:
        client = FakeClient(
            {
                "id": "PL-different",
                "title": "Different",
                "owned": True,
                "privacy": "PRIVATE",
                "tracks": [],
            }
        )
        with patch.object(
            nocky_youtube_playlist_create.nocky_youtube,
            "_load_session",
            return_value={"headers": {"test": "value"}},
        ):
            with patch.object(
                nocky_youtube_playlist_create.nocky_youtube,
                "_create_client",
                return_value=client,
            ):
                with self.assertRaisesRegex(RuntimeError, "mismatched"):
                    nocky_youtube_playlist_create.fetch_playlist_metadata(
                        {"playlist_id": "PL-owned"}
                    )

    def test_unknown_operation_is_rejected_without_session_access(self) -> None:
        with patch.object(
            nocky_youtube_playlist_create.nocky_youtube,
            "_load_session",
        ) as load_session:
            with self.assertRaisesRegex(RuntimeError, "Unsupported"):
                nocky_youtube_playlist_create.execute({"operation": "delete"})
            load_session.assert_not_called()


if __name__ == "__main__":
    unittest.main()
