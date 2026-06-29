from __future__ import annotations

import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "helpers"))

from nocky_account_profile import (  # noqa: E402
    PROFILE_KEYS,
    normalize_account_profile,
    profile_from_session,
    profile_storage_payload,
)


class YouTubeAccountProfileTests(unittest.TestCase):
    def test_normalizes_current_ytmusicapi_shape(self) -> None:
        profile = normalize_account_profile(
            {
                "accountName": "Maylton",
                "channelHandle": "@maylton",
                "accountPhotoUrl": "https://example.invalid/avatar.jpg",
            }
        )

        self.assertEqual(
            profile,
            {
                "name": "Maylton",
                "channel_handle": "@maylton",
                "photo_url": "https://example.invalid/avatar.jpg",
            },
        )

    def test_accepts_legacy_nested_account_shape(self) -> None:
        profile = normalize_account_profile(
            {
                "accounts": [
                    {
                        "accountName": "Legacy profile",
                        "channelHandle": "@legacy",
                        "accountPhotoUrl": "https://example.invalid/legacy.jpg",
                    }
                ]
            }
        )

        self.assertEqual(profile["name"], "Legacy profile")
        self.assertEqual(profile["channel_handle"], "@legacy")
        self.assertEqual(profile["photo_url"], "https://example.invalid/legacy.jpg")

    def test_existing_session_uses_legacy_account_name(self) -> None:
        profile = profile_from_session(
            {
                "headers": {"cookie": "secret", "authorization": "secret"},
                "account": "Existing account",
                "storage": "secret-service",
            }
        )

        self.assertEqual(profile["name"], "Existing account")
        self.assertEqual(profile["channel_handle"], "")
        self.assertEqual(profile["photo_url"], "")

    def test_storage_payload_never_copies_authentication_material(self) -> None:
        payload = profile_storage_payload(
            {
                "accountName": "Safe profile",
                "channelHandle": "@safe",
                "accountPhotoUrl": "https://example.invalid/safe.jpg",
                "cookie": "must-not-leak",
                "authorization": "must-not-leak",
                "headers": {"cookie": "must-not-leak"},
            }
        )

        self.assertEqual(tuple(payload), PROFILE_KEYS)
        self.assertNotIn("cookie", payload)
        self.assertNotIn("authorization", payload)
        self.assertNotIn("headers", payload)

    def test_missing_or_invalid_metadata_degrades_to_empty_profile(self) -> None:
        self.assertEqual(
            normalize_account_profile(None),
            {"name": "", "channel_handle": "", "photo_url": ""},
        )
        self.assertEqual(
            profile_from_session({}),
            {"name": "", "channel_handle": "", "photo_url": ""},
        )


if __name__ == "__main__":
    unittest.main()
