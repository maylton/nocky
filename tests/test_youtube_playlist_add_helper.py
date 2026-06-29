from __future__ import annotations

import io
import json
import sys
import unittest
from pathlib import Path
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "helpers"))

import nocky_youtube_playlist_add  # noqa: E402


class FakeClient:
    def __init__(
        self,
        result="STATUS_SUCCEEDED",
        metadata=None,
        *,
        expose_adder=True,
    ):
        self.result = result
        self.metadata = metadata or {
            "id": "PL-example",
            "title": "Example",
            "owned": True,
            "privacy": "PRIVATE",
            "tracks": [],
        }
        self.metadata_calls = []
        self.calls = []
        if not expose_adder:
            self.add_playlist_items = None

    def get_playlist(self, playlist_id, limit=100):
        self.metadata_calls.append((playlist_id, limit))
        return self.metadata

    def add_playlist_items(self, playlist_id, videoIds=None, duplicates=False):
        self.calls.append((playlist_id, list(videoIds or []), duplicates))
        return self.result


class PlaylistAddHelperTests(unittest.TestCase):
    def test_invalid_request_fails_before_session_access(self) -> None:
        with patch.object(
            nocky_youtube_playlist_add.nocky_youtube,
            "_load_session",
        ) as load_session:
            with self.assertRaisesRegex(RuntimeError, "ownership and editability"):
                nocky_youtube_playlist_add.add_playlist_item(
                    {
                        "playlist_id": "PL-example",
                        "video_id": "abcdefghijk",
                        "owned": False,
                        "editable": True,
                    }
                )
            load_session.assert_not_called()

    def test_disconnected_session_blocks_client_creation(self) -> None:
        with patch.object(
            nocky_youtube_playlist_add.nocky_youtube,
            "_load_session",
            return_value={},
        ):
            with patch.object(
                nocky_youtube_playlist_add.nocky_youtube,
                "_create_client",
            ) as create_client:
                with self.assertRaisesRegex(RuntimeError, "Connect a YouTube Music"):
                    nocky_youtube_playlist_add.add_playlist_item(
                        {
                            "playlist_id": "PL-example",
                            "video_id": "abcdefghijk",
                            "owned": True,
                            "editable": True,
                        }
                    )
                create_client.assert_not_called()

    def test_calls_add_api_once_after_remote_ownership_check(self) -> None:
        client = FakeClient(
            {
                "status": "STATUS_SUCCEEDED",
                "playlistEditResults": [
                    {"videoId": "abcdefghijk", "setVideoId": "not-exposed"}
                ],
            }
        )
        with patch.object(
            nocky_youtube_playlist_add.nocky_youtube,
            "_load_session",
            return_value={"headers": {"test": "value"}},
        ):
            with patch.object(
                nocky_youtube_playlist_add.nocky_youtube,
                "_create_client",
                return_value=client,
            ) as create_client:
                result = nocky_youtube_playlist_add.add_playlist_item(
                    {
                        "playlist_id": "VLPL-example",
                        "video_id": "abcdefghijk",
                        "owned": True,
                        "editable": True,
                    }
                )

        create_client.assert_called_once_with(authenticated=True)
        self.assertEqual(client.metadata_calls, [("PL-example", 1)])
        self.assertEqual(client.calls, [("PL-example", ["abcdefghijk"], False)])
        self.assertEqual(result["added_count"], 1)
        self.assertTrue(result["reconciliation_required"])
        self.assertNotIn("setVideoId", str(result))

    def test_remote_unowned_playlist_blocks_mutation(self) -> None:
        client = FakeClient(
            metadata={
                "id": "PL-example",
                "title": "Shared",
                "owned": False,
                "privacy": "UNLISTED",
                "tracks": [],
            }
        )
        with patch.object(
            nocky_youtube_playlist_add.nocky_youtube,
            "_load_session",
            return_value={"headers": {"test": "value"}},
        ):
            with patch.object(
                nocky_youtube_playlist_add.nocky_youtube,
                "_create_client",
                return_value=client,
            ):
                with self.assertRaisesRegex(RuntimeError, "did not confirm"):
                    nocky_youtube_playlist_add.add_playlist_item(
                        {
                            "playlist_id": "PL-example",
                            "video_id": "abcdefghijk",
                            "owned": True,
                            "editable": True,
                        }
                    )

        self.assertEqual(client.metadata_calls, [("PL-example", 1)])
        self.assertEqual(client.calls, [])

    def test_mismatched_remote_playlist_blocks_mutation(self) -> None:
        client = FakeClient(
            metadata={
                "id": "PL-different",
                "title": "Different",
                "owned": True,
                "privacy": "PRIVATE",
                "tracks": [],
            }
        )
        with patch.object(
            nocky_youtube_playlist_add.nocky_youtube,
            "_load_session",
            return_value={"headers": {"test": "value"}},
        ):
            with patch.object(
                nocky_youtube_playlist_add.nocky_youtube,
                "_create_client",
                return_value=client,
            ):
                with self.assertRaisesRegex(RuntimeError, "mismatched"):
                    nocky_youtube_playlist_add.add_playlist_item(
                        {
                            "playlist_id": "PL-example",
                            "video_id": "abcdefghijk",
                            "owned": True,
                            "editable": True,
                        }
                    )

        self.assertEqual(client.calls, [])

    def test_missing_read_method_is_sanitized(self) -> None:
        with patch.object(
            nocky_youtube_playlist_add.nocky_youtube,
            "_load_session",
            return_value={"headers": {"test": "value"}},
        ):
            with patch.object(
                nocky_youtube_playlist_add.nocky_youtube,
                "_create_client",
                return_value=object(),
            ):
                with self.assertRaisesRegex(RuntimeError, "cannot verify playlist ownership"):
                    nocky_youtube_playlist_add.add_playlist_item(
                        {
                            "playlist_id": "PL-example",
                            "video_id": "abcdefghijk",
                            "owned": True,
                            "editable": True,
                        }
                    )

    def test_missing_add_method_is_sanitized_after_verification(self) -> None:
        client = FakeClient(expose_adder=False)
        with patch.object(
            nocky_youtube_playlist_add.nocky_youtube,
            "_load_session",
            return_value={"headers": {"test": "value"}},
        ):
            with patch.object(
                nocky_youtube_playlist_add.nocky_youtube,
                "_create_client",
                return_value=client,
            ):
                with self.assertRaisesRegex(RuntimeError, "cannot add playlist items"):
                    nocky_youtube_playlist_add.add_playlist_item(
                        {
                            "playlist_id": "PL-example",
                            "video_id": "abcdefghijk",
                            "owned": True,
                            "editable": True,
                        }
                    )

        self.assertEqual(client.metadata_calls, [("PL-example", 1)])
        self.assertEqual(client.calls, [])

    def test_main_emits_standard_json_error_envelope(self) -> None:
        output = io.StringIO()
        with patch.object(sys, "stdin", io.StringIO("{}")):
            with patch.object(sys, "stdout", output):
                status = nocky_youtube_playlist_add.main()

        payload = json.loads(output.getvalue())
        self.assertEqual(status, 2)
        self.assertFalse(payload["ok"])
        self.assertIn("ownership", payload["error"])


if __name__ == "__main__":
    unittest.main()
