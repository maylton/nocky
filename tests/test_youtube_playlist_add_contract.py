from __future__ import annotations

import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "helpers"))

from nocky_playlist_add_contract import (  # noqa: E402
    normalize_add_request,
    sanitize_add_result,
)


class PlaylistAddContractTests(unittest.TestCase):
    def test_normalizes_one_owned_editable_item(self) -> None:
        self.assertEqual(
            normalize_add_request(
                {
                    "playlist_id": "VLPL-example_123",
                    "video_id": "abcdefghijk",
                    "owned": True,
                    "editable": True,
                }
            ),
            {
                "playlist_id": "PL-example_123",
                "video_ids": ["abcdefghijk"],
                "duplicates": False,
            },
        )

    def test_requires_confirmed_ownership_and_editability(self) -> None:
        for owned, editable in ((False, True), (True, False), (False, False)):
            with self.subTest(owned=owned, editable=editable):
                with self.assertRaisesRegex(RuntimeError, "ownership and editability"):
                    normalize_add_request(
                        {
                            "playlist_id": "PL-example",
                            "video_id": "abcdefghijk",
                            "owned": owned,
                            "editable": editable,
                        }
                    )

    def test_rejects_urls_batch_source_playlist_and_duplicates(self) -> None:
        invalid = (
            {
                "playlist_id": "https://music.youtube.com/playlist?list=PL-example",
                "video_id": "abcdefghijk",
                "owned": True,
                "editable": True,
            },
            {
                "playlist_id": "PL-example",
                "video_ids": ["abcdefghijk", "lmnopqrstuv"],
                "owned": True,
                "editable": True,
            },
            {
                "playlist_id": "PL-example",
                "video_id": "abcdefghijk",
                "source_playlist": "PL-source",
                "owned": True,
                "editable": True,
            },
            {
                "playlist_id": "PL-example",
                "video_id": "abcdefghijk",
                "duplicates": True,
                "owned": True,
                "editable": True,
            },
        )
        for payload in invalid:
            with self.subTest(payload=payload):
                with self.assertRaises(RuntimeError):
                    normalize_add_request(payload)

    def test_rejects_malformed_video_id(self) -> None:
        with self.assertRaisesRegex(RuntimeError, "Invalid YouTube video ID"):
            normalize_add_request(
                {
                    "playlist_id": "PL-example",
                    "video_id": "too-short",
                    "owned": True,
                    "editable": True,
                }
            )

    def test_sanitizes_success_without_remote_identity_details(self) -> None:
        result = sanitize_add_result(
            {
                "status": "STATUS_SUCCEEDED",
                "playlistEditResults": [
                    {"videoId": "abcdefghijk", "setVideoId": "private-set-id"}
                ],
                "private_field": "ignored",
            },
            playlist_id="PL-example",
            video_id="abcdefghijk",
        )

        self.assertEqual(
            result,
            {
                "playlist_id": "PL-example",
                "video_id": "abcdefghijk",
                "added_count": 1,
                "reconciliation_required": True,
            },
        )
        self.assertNotIn("setVideoId", str(result))
        self.assertNotIn("private_field", str(result))

    def test_rejects_unconfirmed_remote_result(self) -> None:
        for result in ({}, {"status": "STATUS_FAILED"}, "STATUS_FAILED", None):
            with self.subTest(result=result):
                with self.assertRaisesRegex(RuntimeError, "did not confirm"):
                    sanitize_add_result(
                        result,
                        playlist_id="PL-example",
                        video_id="abcdefghijk",
                    )


if __name__ == "__main__":
    unittest.main()
