from __future__ import annotations

import sys
import unittest
from pathlib import Path
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "helpers"))

import nocky_youtube_profiles  # noqa: E402


class FakeClient:
    def __init__(self, response):
        self.response = response
        self.calls = []

    def _send_request(self, endpoint, body):
        self.calls.append((endpoint, body))
        return self.response


class YouTubeProfilesHelperTests(unittest.TestCase):
    def test_requests_accounts_list_read_only_endpoint(self) -> None:
        client = FakeClient({"actions": []})

        result = nocky_youtube_profiles._raw_accounts_list(client)

        self.assertEqual(result, {"actions": []})
        self.assertEqual(client.calls, [("account/accounts_list", {})])

    def test_rejects_invalid_response(self) -> None:
        client = FakeClient([])

        with self.assertRaisesRegex(RuntimeError, "invalid account-list response"):
            nocky_youtube_profiles._raw_accounts_list(client)

    def test_requires_connected_session_before_network_access(self) -> None:
        with patch.object(nocky_youtube_profiles.nocky_youtube, "_load_session", return_value={}):
            with patch.object(nocky_youtube_profiles.nocky_youtube, "_create_client") as create:
                with self.assertRaisesRegex(RuntimeError, "Connect a YouTube Music"):
                    nocky_youtube_profiles.discover_profiles()
                create.assert_not_called()

    def test_returns_only_sanitized_discovery_contract(self) -> None:
        response = {
            "actions": [
                {
                    "getMultiPageMenuAction": {
                        "menu": {
                            "multiPageMenuRenderer": {
                                "sections": [
                                    {
                                        "accountSectionListRenderer": {
                                            "header": {"private_field": "ignored"},
                                            "contents": [
                                                {
                                                    "accountItemSectionRenderer": {
                                                        "contents": [
                                                            {
                                                                "accountItem": {
                                                                    "accountName": {
                                                                        "runs": [{"text": "Primary"}]
                                                                    },
                                                                    "channelHandle": {
                                                                        "runs": [{"text": "@primary"}]
                                                                    },
                                                                    "isSelected": True,
                                                                    "extra_field": "ignored",
                                                                }
                                                            }
                                                        ]
                                                    }
                                                }
                                            ],
                                        }
                                    }
                                ]
                            }
                        }
                    }
                }
            ],
            "private_context": "ignored",
        }
        client = FakeClient(response)

        with patch.object(
            nocky_youtube_profiles.nocky_youtube,
            "_load_session",
            return_value={"headers": {"test_header": "value"}},
        ):
            with patch.object(
                nocky_youtube_profiles.nocky_youtube,
                "_create_client",
                return_value=client,
            ):
                result = nocky_youtube_profiles.discover_profiles()

        self.assertEqual(result["state"], "single")
        self.assertEqual(result["profiles"][0]["profile_id"], "primary")
        self.assertNotIn("private_field", str(result))
        self.assertNotIn("extra_field", str(result))
        self.assertNotIn("private_context", str(result))

    def test_native_summary_exposes_counts_only(self) -> None:
        summary = nocky_youtube_profiles.discovery_summary(
            {
                "state": "multiple",
                "deterministic": True,
                "profiles": [
                    {
                        "profile_id": "profile-a",
                        "name": "Primary",
                        "channel_handle": "@primary",
                        "photo_url": "https://example.invalid/primary.jpg",
                        "is_selected": True,
                    },
                    {
                        "profile_id": "profile-b",
                        "name": "Brand",
                        "channel_handle": "@brand",
                        "photo_url": "https://example.invalid/brand.jpg",
                        "is_selected": False,
                    },
                ],
            }
        )

        self.assertEqual(
            summary,
            {
                "state": "multiple",
                "deterministic": True,
                "profile_count": 2,
            },
        )
        serialized = str(summary)
        for forbidden in (
            "profile-a",
            "profile-b",
            "example.invalid",
            "Primary",
            "@primary",
            "Brand",
            "@brand",
        ):
            self.assertNotIn(forbidden, serialized)

    def test_native_summary_degrades_to_unavailable(self) -> None:
        self.assertEqual(
            nocky_youtube_profiles.discovery_summary(None),
            {
                "state": "unavailable",
                "deterministic": False,
                "profile_count": 0,
            },
        )


if __name__ == "__main__":
    unittest.main()
