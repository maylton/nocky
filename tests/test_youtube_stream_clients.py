#!/usr/bin/env python3
from __future__ import annotations

import os
from pathlib import Path
import sys
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "helpers"))

from nocky_stream_clients import (  # noqa: E402
    PROFILES,
    build_attempt_command,
    concise_process_error,
    error_category,
    ordered_profiles,
    parse_requested_order,
    policy_snapshot,
    should_try_next_client,
)


class StreamClientPolicyTests(unittest.TestCase):
    def test_default_order_uses_supported_clients(self) -> None:
        with patch.dict(os.environ, {}, clear=True):
            order = [profile.key for profile in ordered_profiles(has_auth=True)]
        self.assertEqual(order, ["web_music", "web_creator", "tv", "android_vr", "web"])

    def test_unauthenticated_order_skips_creator(self) -> None:
        with patch.dict(os.environ, {}, clear=True):
            order = [profile.key for profile in ordered_profiles(has_auth=False)]
        self.assertNotIn("web_creator", order)
        self.assertIn("android_vr", order)

    def test_failed_client_moves_to_end_for_recovery(self) -> None:
        order = [
            profile.key
            for profile in ordered_profiles(
                has_auth=True,
                failed_client="web_music",
                requested_order=["web_music", "tv", "android_vr"],
            )
        ]
        self.assertEqual(order, ["tv", "android_vr", "web_music"])

    def test_explicit_order_allows_disabled_ios_profile(self) -> None:
        self.assertEqual(parse_requested_order("ios,tv,ios,unknown"), ["ios", "tv"])
        order = [
            profile.key
            for profile in ordered_profiles(
                has_auth=False,
                requested_order=["ios", "tv"],
            )
        ]
        self.assertEqual(order, ["ios", "tv"])

    def test_command_adds_only_profile_appropriate_auth(self) -> None:
        base = ["yt-dlp", "--dump-single-json", "https://example.invalid/watch"]
        auth = ["--cookies", "/tmp/cookies"]
        web_music = build_attempt_command(
            base,
            base[-1],
            PROFILES["web_music"],
            auth,
        )
        android_vr = build_attempt_command(
            base,
            base[-1],
            PROFILES["android_vr"],
            auth,
        )
        self.assertIn("/tmp/cookies", web_music)
        self.assertNotIn("/tmp/cookies", android_vr)
        self.assertIn("youtube:player_client=android_vr", android_vr)

    def test_terminal_errors_do_not_rotate_clients(self) -> None:
        self.assertEqual(error_category("This video is private"), "terminal")
        self.assertFalse(should_try_next_client("This video is private"))
        self.assertTrue(should_try_next_client("HTTP Error 403: Forbidden"))

    def test_diagnostics_redact_urls_and_headers(self) -> None:
        detail = concise_process_error(
            "ERROR https://rr1.googlevideo.com/path?token=secret\nCookie: abc"
        )
        self.assertNotIn("secret", detail)
        self.assertNotIn("abc", detail)
        self.assertIn("<redacted-url>", detail)

    def test_snapshot_marks_auth_requirements(self) -> None:
        snapshot = policy_snapshot(has_auth=False)
        creator = next(item for item in snapshot["clients"] if item["key"] == "web_creator")
        self.assertFalse(creator["available"])
        self.assertFalse(creator["uses_auth"])


if __name__ == "__main__":
    unittest.main()
