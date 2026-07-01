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
    def __init__(
        self,
        response=None,
        add_result="STATUS_SUCCEEDED",
        edit_result="STATUS_SUCCEEDED",
    ):
        self.response = response or {
            "id": "PL-owned",
            "title": "Focus",
            "owned": True,
            "privacy": "PRIVATE",
            "tracks": [
                {
                    "videoId": "zzzzzzzzzzz",
                    "setVideoId": "set-occurrence-1",
                    "title": "Track",
                }
            ],
        }
        self.add_result = add_result
        self.edit_result = edit_result
        self.calls = []
        self.add_calls = []
        self.edit_calls = []

    def get_playlist(self, playlist_id, limit=100):
        self.calls.append((playlist_id, limit))
        return self.response

    def add_playlist_items(self, playlist_id, videoIds=None, duplicates=False):
        self.add_calls.append((playlist_id, list(videoIds or []), duplicates))
        return self.add_result

    def edit_playlist(
        self,
        playlist_id,
        title=None,
        description=None,
        privacyStatus=None,
    ):
        self.edit_calls.append((playlist_id, title, description, privacyStatus))
        return self.edit_result


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

    def test_add_operation_revalidates_and_submits_exactly_one_item(self) -> None:
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
            ):
                result = nocky_youtube_playlist_create.execute(
                    {
                        "operation": "add",
                        "playlist_id": "VLPL-owned",
                        "video_id": "abcdefghijk",
                        "owned": True,
                        "editable": True,
                    }
                )

        self.assertEqual(client.calls, [("PL-owned", 500)])
        self.assertEqual(client.add_calls, [("PL-owned", ["abcdefghijk"], False)])
        self.assertEqual(
            result,
            {
                "playlist_id": "PL-owned",
                "video_id": "abcdefghijk",
                "added_count": 1,
                "reconciliation_required": True,
            },
        )

    def test_add_rejects_duplicate_or_unowned_requests_before_session_access(self) -> None:
        invalid = (
            {
                "operation": "add",
                "playlist_id": "PL-owned",
                "video_id": "abcdefghijk",
                "owned": False,
                "editable": True,
            },
            {
                "operation": "add",
                "playlist_id": "PL-owned",
                "video_id": "abcdefghijk",
                "owned": True,
                "editable": True,
                "duplicates": True,
            },
        )
        with patch.object(
            nocky_youtube_playlist_create.nocky_youtube,
            "_load_session",
        ) as load_session:
            for payload in invalid:
                with self.subTest(payload=payload):
                    with self.assertRaises(RuntimeError):
                        nocky_youtube_playlist_create.execute(payload)
            load_session.assert_not_called()

    def test_existing_remote_item_blocks_addition(self) -> None:
        client = FakeClient(
            {
                "id": "PL-owned",
                "title": "Focus",
                "owned": True,
                "privacy": "PRIVATE",
                "tracks": [{"videoId": "abcdefghijk", "title": "Already there"}],
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
                with self.assertRaisesRegex(RuntimeError, "already"):
                    nocky_youtube_playlist_create.execute(
                        {
                            "operation": "add",
                            "playlist_id": "PL-owned",
                            "video_id": "abcdefghijk",
                            "owned": True,
                            "editable": True,
                        }
                    )
        self.assertEqual(client.add_calls, [])

    def test_remote_shared_playlist_blocks_addition(self) -> None:
        client = FakeClient(
            {
                "id": "PL-owned",
                "title": "Shared",
                "owned": False,
                "privacy": "UNLISTED",
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
                with self.assertRaisesRegex(RuntimeError, "ownership and editability"):
                    nocky_youtube_playlist_create.execute(
                        {
                            "operation": "add",
                            "playlist_id": "PL-owned",
                            "video_id": "abcdefghijk",
                            "owned": True,
                            "editable": True,
                        }
                    )
        self.assertEqual(client.add_calls, [])

    def test_metadata_edit_revalidates_and_calls_edit_playlist(self) -> None:
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
            ):
                result = nocky_youtube_playlist_create.execute(
                    {
                        "operation": "edit_metadata",
                        "playlist_id": "VLPL-owned",
                        "owned": True,
                        "editable": True,
                        "current": {
                            "title": "Focus",
                            "description": "",
                            "privacy": "PRIVATE",
                        },
                        "title": "Deep Focus",
                        "privacy": "UNLISTED",
                    }
                )

        self.assertEqual(client.calls, [("PL-owned", 1)])
        self.assertEqual(client.edit_calls, [("PL-owned", "Deep Focus", None, "UNLISTED")])
        self.assertEqual(
            result,
            {
                "playlist_id": "PL-owned",
                "title": "Deep Focus",
                "privacy": "UNLISTED",
                "reconciliation_required": True,
            },
        )

    def test_metadata_edit_rejects_noop_or_unowned_before_session_access(self) -> None:
        invalid = (
            {
                "operation": "edit_metadata",
                "playlist_id": "PL-owned",
                "owned": True,
                "editable": True,
                "current": {"title": "Focus", "privacy": "PRIVATE"},
                "title": "Focus",
                "privacy": "PRIVATE",
            },
            {
                "operation": "edit_metadata",
                "playlist_id": "PL-owned",
                "owned": False,
                "editable": True,
                "current": {"title": "Focus", "privacy": "PRIVATE"},
                "title": "Deep Focus",
            },
            {
                "operation": "edit_metadata",
                "playlist_id": "PL-owned",
                "owned": True,
                "editable": True,
                "current": {"title": "Focus", "privacy": "PRIVATE"},
                "title": "Bad <title>",
            },
        )
        with patch.object(
            nocky_youtube_playlist_create.nocky_youtube,
            "_load_session",
        ) as load_session:
            for payload in invalid:
                with self.subTest(payload=payload):
                    with self.assertRaises(RuntimeError):
                        nocky_youtube_playlist_create.execute(payload)
            load_session.assert_not_called()

    def test_metadata_edit_blocks_stale_or_shared_remote_playlist(self) -> None:
        stale = FakeClient(
            {
                "id": "PL-owned",
                "title": "Renamed elsewhere",
                "owned": True,
                "privacy": "PRIVATE",
                "tracks": [],
            }
        )
        shared = FakeClient(
            {
                "id": "PL-owned",
                "title": "Focus",
                "owned": False,
                "privacy": "PRIVATE",
                "tracks": [],
            }
        )
        for client, message in (
            (stale, "metadata changed"),
            (shared, "ownership and editability"),
        ):
            with self.subTest(message=message):
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
                        with self.assertRaisesRegex(RuntimeError, message):
                            nocky_youtube_playlist_create.execute(
                                {
                                    "operation": "edit_metadata",
                                    "playlist_id": "PL-owned",
                                    "owned": True,
                                    "editable": True,
                                    "current": {
                                        "title": "Focus",
                                        "privacy": "PRIVATE",
                                    },
                                    "title": "Deep Focus",
                                }
                            )
                self.assertEqual(client.edit_calls, [])

    def test_generated_playlist_alias_is_read_only(self) -> None:
        client = FakeClient(
            {
                "id": "RD-canonical",
                "title": "Dynamic mix",
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
                result = nocky_youtube_playlist_create.fetch_playlist_metadata(
                    {"playlist_id": "RDTMAK5uy_dynamic"}
                )

        self.assertEqual(result["playlist_id"], "RDTMAK5uy_dynamic")
        self.assertFalse(result["owned"])
        self.assertFalse(result["editable"])

    def test_generated_alias_is_still_rejected_for_mutation(self) -> None:
        client = FakeClient(
            {
                "id": "RD-canonical",
                "title": "Dynamic mix",
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
                    nocky_youtube_playlist_create.add_playlist_item(
                        {
                            "playlist_id": "RDTMAK5uy_dynamic",
                            "video_id": "abcdefghijk",
                            "owned": True,
                            "editable": True,
                        }
                    )
        self.assertEqual(client.add_calls, [])

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
