#!/usr/bin/env python3
from __future__ import annotations

import json
import os
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "helpers"))

from nocky_stream_clients import (  # noqa: E402
    ENV_CONFIG_FILE,
    ENV_ORDER,
    PROFILES,
    build_attempt_command,
    concise_process_error,
    configured_order,
    error_category,
    ordered_profiles,
    parse_requested_order,
    policy_snapshot,
    should_try_next_client,
)


class StreamClientPolicyTests(unittest.TestCase):
    def isolated_environment(self, root: str) -> dict[str, str]:
        return {ENV_CONFIG_FILE: str(Path(root) / "missing-config.json")}

    def test_default_order_uses_supported_clients(self) -> None:
        with tempfile.TemporaryDirectory() as root:
            with patch.dict(os.environ, self.isolated_environment(root), clear=True):
                order = [profile.key for profile in ordered_profiles(has_auth=True)]
        self.assertEqual(order, ["web_music", "web_creator", "tv", "android_vr", "web"])

    def test_unauthenticated_order_skips_creator(self) -> None:
        with tempfile.TemporaryDirectory() as root:
            with patch.dict(os.environ, self.isolated_environment(root), clear=True):
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

    def test_persisted_order_and_disabled_sources_are_applied(self) -> None:
        with tempfile.TemporaryDirectory() as root:
            config_path = Path(root) / "config.json"
            config_path.write_text(
                json.dumps(
                    {
                        "youtube_stream_sources": {
                            "order": ["ios", "tv", "unknown", "ios"],
                            "disabled": ["tv", "web_creator"],
                        }
                    }
                ),
                encoding="utf-8",
            )
            with patch.dict(
                os.environ,
                {ENV_CONFIG_FILE: str(config_path)},
                clear=True,
            ):
                self.assertEqual(
                    configured_order(),
                    ["ios", "web_music", "android_vr", "web"],
                )
                order = [
                    profile.key for profile in ordered_profiles(has_auth=True)
                ]
        self.assertEqual(order, ["ios", "web_music", "android_vr", "web"])

    def test_environment_order_has_priority_over_persisted_config(self) -> None:
        with tempfile.TemporaryDirectory() as root:
            config_path = Path(root) / "config.json"
            config_path.write_text(
                json.dumps(
                    {
                        "youtube_stream_sources": {
                            "order": ["ios", "tv"],
                            "disabled": [],
                        }
                    }
                ),
                encoding="utf-8",
            )
            with patch.dict(
                os.environ,
                {
                    ENV_CONFIG_FILE: str(config_path),
                    ENV_ORDER: "android_vr,web",
                },
                clear=True,
            ):
                self.assertEqual(
                    parse_requested_order(None),
                    ["android_vr", "web"],
                )

    def test_invalid_config_falls_back_to_defaults(self) -> None:
        with tempfile.TemporaryDirectory() as root:
            config_path = Path(root) / "config.json"
            config_path.write_text("not json", encoding="utf-8")
            with patch.dict(
                os.environ,
                {ENV_CONFIG_FILE: str(config_path)},
                clear=True,
            ):
                self.assertIsNone(configured_order())
                self.assertEqual(
                    parse_requested_order(None),
                    ["web_music", "web_creator", "tv", "android_vr", "web"],
                )

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
        with tempfile.TemporaryDirectory() as root:
            with patch.dict(os.environ, self.isolated_environment(root), clear=True):
                snapshot = policy_snapshot(has_auth=False)
        creator = next(item for item in snapshot["clients"] if item["key"] == "web_creator")
        self.assertFalse(creator["available"])
        self.assertFalse(creator["uses_auth"])


if __name__ == "__main__":
    unittest.main()
